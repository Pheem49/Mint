//! Shared text preparation, bounded audio delivery and cross-process playback ownership.
use super::google_tts_urls;
use crate::MemoryStore;
use base64::Engine;
pub use mint_companion_protocol::{SpeechReply, SpeechRequest};
use rusqlite::{OptionalExtension, params};
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const LEASE_SECONDS: i64 = 12;
const AUTO_LIMIT: usize = 1200;
const MANUAL_LIMIT: usize = 8000;
const MAX_AUDIO_BYTES: usize = 1_048_576;
#[derive(Clone)]
struct Plan {
    urls: Vec<String>,
}
static PLANS: OnceLock<Mutex<HashMap<String, Plan>>> = OnceLock::new();
fn plans() -> &'static Mutex<HashMap<String, Plan>> {
    PLANS.get_or_init(Default::default)
}
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
fn db(memory: &MemoryStore) -> Result<rusqlite::Connection, String> {
    let connection = memory.connection().map_err(|e| e.to_string())?;
    connection.execute_batch("CREATE TABLE IF NOT EXISTS speech_lease (slot INTEGER PRIMARY KEY CHECK(slot=1), owner TEXT NOT NULL, chat_id TEXT NOT NULL, expires INTEGER NOT NULL);
        CREATE TABLE IF NOT EXISTS speech_auto (chat_id TEXT NOT NULL, turn_id INTEGER NOT NULL, PRIMARY KEY(chat_id,turn_id));
        CREATE TABLE IF NOT EXISTS speech_routes (chat_id TEXT PRIMARY KEY, owner TEXT NOT NULL, expires INTEGER NOT NULL);
        CREATE TABLE IF NOT EXISTS speech_cancelled (owner TEXT PRIMARY KEY, expires INTEGER NOT NULL);").map_err(|e| e.to_string())?;
    Ok(connection)
}
fn owner_valid(owner: &str) -> Result<(), String> {
    uuid::Uuid::parse_str(owner)
        .map(|_| ())
        .map_err(|_| "Invalid speech owner".into())
}

pub fn clean_text(input: &str) -> String {
    let value = serde_json::from_str::<serde_json::Value>(input).ok();
    if value.as_ref().is_some_and(|value| {
        value.is_object()
            && value.pointer("/input/summary").is_none()
            && value.pointer("/finish/summary").is_none()
    }) {
        return String::new();
    }
    let input = value
        .as_ref()
        .and_then(|v| {
            v.pointer("/input/summary")
                .or_else(|| v.pointer("/finish/summary"))
        })
        .and_then(|v| v.as_str())
        .unwrap_or(input);
    let mut fenced = false;
    let lines = input
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.starts_with("```") || line.starts_with("~~~") {
                fenced = !fenced;
                return None;
            }
            if fenced || line.starts_with('|') {
                None
            } else {
                Some(line)
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    let mut text = lines;
    for (pattern, replacement) in [
        (r"(?s)<think(?:ing)?>.*?</think(?:ing)?>", " "),
        (r"!?\[([^\]]*)\]\([^)]*\)", "$1"),
        (r"`+[^`]*`+", " "),
        (r"(?:https?://|www\.)\S+", " "),
        (r"<[^>]+>", " "),
        (r"[*_#~]", ""),
    ] {
        text = regex::Regex::new(pattern)
            .unwrap()
            .replace_all(&text, replacement)
            .into_owned();
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn acquire(
    memory: &MemoryStore,
    owner: &str,
    chat: &str,
    automatic_turn: Option<i64>,
    route_owner: Option<&str>,
) -> Result<(), String> {
    let mut connection = db(memory)?;
    let tx = connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let stamp = now();
    tx.execute("DELETE FROM speech_cancelled WHERE expires < ?1", [stamp])
        .map_err(|e| e.to_string())?;
    if tx
        .query_row(
            "SELECT 1 FROM speech_cancelled WHERE owner=?1",
            [owner],
            |_| Ok(()),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .is_some()
    {
        return Err("Speech was stopped".into());
    }
    // Serialize the latest-turn check with ownership acquisition. A new turn either
    // already exists here or will revoke this lease after its own database write.
    if let Some(id) = automatic_turn {
        let newer: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM interaction_memories WHERE chat_id=?1 AND id>?2)",
                params![chat, id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if newer {
            return Err("A newer message has arrived".into());
        }
    }
    if automatic_turn.is_some() {
        let preferred: Option<String> = tx
            .query_row(
                "SELECT owner FROM speech_routes WHERE chat_id=?1 AND expires>?2",
                params![chat, stamp],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if preferred
            .as_deref()
            .is_some_and(|owner| Some(owner) != route_owner)
        {
            return Err("Automatic replies are routed to Companion.".into());
        }
    }
    let active: Option<String> = tx
        .query_row(
            "SELECT owner FROM speech_lease WHERE slot=1 AND expires>?1",
            [stamp],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if active.is_some() {
        return Err("Another Mint surface is speaking. Stop it before playing here.".into());
    }
    if let Some(id) = automatic_turn {
        let inserted = tx
            .execute(
                "INSERT OR IGNORE INTO speech_auto(chat_id,turn_id) VALUES(?1,?2)",
                params![chat, id],
            )
            .map_err(|e| e.to_string())?;
        if inserted == 0 {
            return Err("This reply has already been read automatically.".into());
        }
    }
    tx.execute(
        "INSERT OR REPLACE INTO speech_lease(slot,owner,chat_id,expires) VALUES(1,?1,?2,?3)",
        params![owner, chat, stamp + LEASE_SECONDS],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}
fn active(memory: &MemoryStore, owner: &str) -> Result<bool, String> {
    Ok(db(memory)?
        .query_row(
            "SELECT 1 FROM speech_lease WHERE slot=1 AND owner=?1 AND expires>?2",
            params![owner, now()],
            |_| Ok(()),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .is_some())
}
// A revoked owner retains its lease until acknowledgement/expiry, preventing an old final audio segment from overlapping a new player.
pub fn cancel_chat(memory: &MemoryStore, chat: &str) {
    if let Ok(connection) = db(memory) {
        let _ = connection.execute(
            "UPDATE speech_lease SET owner='!' || owner WHERE chat_id=?1 AND owner NOT LIKE '!%'",
            [chat],
        );
    }
}
fn setting(config: &crate::MintConfig, key: &str, default: f64, min: f64, max: f64) -> f64 {
    config
        .extra
        .get(key)
        .and_then(|v| v.as_f64().or_else(|| v.as_str()?.parse().ok()))
        .filter(|v| v.is_finite())
        .unwrap_or(default)
        .clamp(min, max)
}

pub async fn request(request: SpeechRequest) -> Result<SpeechReply, String> {
    let memory = MemoryStore::open_default().map_err(|e| e.to_string())?;
    handle(&memory, request).await
}
async fn handle(memory: &MemoryStore, request: SpeechRequest) -> Result<SpeechReply, String> {
    match request {
        SpeechRequest::Prefer {
            owner,
            chat_id,
            enabled,
        } => {
            owner_valid(&owner)?;
            let mut connection = db(memory)?;
            let tx = connection
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(|e| e.to_string())?;
            if !enabled {
                tx.execute(
                    "INSERT OR REPLACE INTO speech_cancelled(owner,expires) VALUES(?1,?2)",
                    params![owner, now() + 60],
                )
                .map_err(|e| e.to_string())?;
                tx.execute(
                    "DELETE FROM speech_routes WHERE chat_id=?1 AND owner=?2",
                    params![chat_id, owner],
                )
                .map_err(|e| e.to_string())?;
                tx.commit().map_err(|e| e.to_string())?;
                return Ok(SpeechReply::Lease { active: false });
            }
            let cancelled = tx
                .query_row(
                    "SELECT 1 FROM speech_cancelled WHERE owner=?1 AND expires>?2",
                    params![owner, now()],
                    |_| Ok(()),
                )
                .optional()
                .map_err(|e| e.to_string())?
                .is_some();
            let current: Option<String> = tx
                .query_row(
                    "SELECT owner FROM speech_routes WHERE chat_id=?1 AND expires>?2",
                    params![chat_id, now()],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|e| e.to_string())?;
            if cancelled || current.as_ref().is_some_and(|value| value != &owner) {
                return Ok(SpeechReply::Lease { active: false });
            }
            tx.execute(
                "INSERT OR REPLACE INTO speech_routes(chat_id,owner,expires) VALUES(?1,?2,?3)",
                params![chat_id, owner, now() + 12],
            )
            .map_err(|e| e.to_string())?;
            tx.commit().map_err(|e| e.to_string())?;
            Ok(SpeechReply::Lease { active: true })
        }

        SpeechRequest::Prepare {
            owner,
            chat_id,
            turn_id,
            text,
            automatic,
            audio_only,
            route_owner,
        } => {
            owner_valid(&owner)?;
            let input = if let Some(id) = turn_id {
                let connection = memory.connection().map_err(|e| e.to_string())?;
                connection.query_row("SELECT ai_text FROM interaction_memories WHERE id=?1 AND chat_id=?2 AND status='completed'", params![id,chat_id], |r| r.get::<_,String>(0)).optional().map_err(|e| e.to_string())?.ok_or("Only a completed saved reply can be read")?
            } else {
                if automatic || audio_only {
                    return Err("Choose a completed saved reply".into());
                }
                text.unwrap_or_default()
            };
            if input.len() > 256_000 {
                return Err("Reply is too large to read".into());
            }
            let text = clean_text(&input);
            if text.is_empty() {
                return Err("This reply has no readable text".into());
            }
            if text.chars().count() > if automatic { AUTO_LIMIT } else { MANUAL_LIMIT } {
                return Err(if automatic {
                    "Long reply: use Read latest reply to play it manually."
                } else {
                    "Reply exceeds the 8,000-character speech limit."
                }
                .into());
            }
            let config = crate::load_config().map_err(|e| e.to_string())?;
            let provider = config
                .extra
                .get("ttsProvider")
                .and_then(|v| v.as_str())
                .unwrap_or("google")
                .to_owned();
            if !matches!(provider.as_str(), "google" | "native") {
                return Err("Unsupported speech provider".into());
            }
            if audio_only && provider == "native" {
                return Err("OS Native cannot send audio to Companion. Select Google voice engine in Mint Audio settings.".into());
            }
            let urls = if provider == "google" {
                google_tts_urls(&text, &config.language)
                    .into_iter()
                    .map(|v| v.url)
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            acquire(
                memory,
                &owner,
                &chat_id,
                if automatic { turn_id } else { None },
                route_owner.as_deref(),
            )?;
            if !automatic && let Some(id) = turn_id {
                db(memory)?
                    .execute(
                        "INSERT OR IGNORE INTO speech_auto(chat_id,turn_id) VALUES(?1,?2)",
                        params![chat_id, id],
                    )
                    .map_err(|e| e.to_string())?;
            }
            let chunks = urls.len();
            let mut plans = plans().lock().unwrap();
            plans.clear();
            plans.insert(owner, Plan { urls });
            Ok(SpeechReply::Prepared {
                text,
                provider,
                volume: setting(&config, "ttsVolume", 1., 0., 1.),
                speed: setting(&config, "ttsSpeed", 1., 0.25, 4.),
                pitch: setting(&config, "ttsPitch", 1., 0., 2.),
                chunks,
            })
        }
        SpeechRequest::Audio { owner, index } => {
            owner_valid(&owner)?;
            if !active(memory, &owner)? {
                return Err("Speech was stopped or playback ownership expired".into());
            }
            let url = plans()
                .lock()
                .unwrap()
                .get(&owner)
                .and_then(|p| p.urls.get(index))
                .cloned()
                .ok_or("Speech audio is no longer available")?;
            let mut response = reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .user_agent("Mozilla/5.0 Mint")
                .build()
                .map_err(|e| e.to_string())?
                .get(url)
                .send()
                .await
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?;
            if !response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| v.starts_with("audio/"))
            {
                return Err("Speech provider returned no audio".into());
            }
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
                if bytes.len() + chunk.len() > MAX_AUDIO_BYTES {
                    return Err("Speech audio chunk is too large".into());
                }
                bytes.extend_from_slice(&chunk);
            }
            if bytes.is_empty() || !active(memory, &owner)? {
                return Err("Speech was stopped or returned empty audio".into());
            }
            Ok(SpeechReply::Audio {
                data: base64::engine::general_purpose::STANDARD.encode(bytes),
            })
        }
        SpeechRequest::Renew { owner } => {
            owner_valid(&owner)?;
            let changed = db(memory)?
                .execute(
                    "UPDATE speech_lease SET expires=?2 WHERE owner=?1 AND expires>?3",
                    params![owner, now() + LEASE_SECONDS, now()],
                )
                .map_err(|e| e.to_string())?;
            Ok(SpeechReply::Lease {
                active: changed > 0,
            })
        }
        SpeechRequest::Release { owner } => {
            owner_valid(&owner)?;
            let connection = db(memory)?;
            connection
                .execute(
                    "INSERT OR REPLACE INTO speech_cancelled(owner,expires) VALUES(?1,?2)",
                    params![owner, now() + 60],
                )
                .map_err(|e| e.to_string())?;
            connection
                .execute(
                    "DELETE FROM speech_lease WHERE owner=?1 OR owner='!' || ?1",
                    [&owner],
                )
                .map_err(|e| e.to_string())?;
            plans().lock().unwrap().remove(&owner);
            Ok(SpeechReply::Released)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prepares_only_readable_prose() {
        assert_eq!(
            clean_text(
                "สวัสดี **Mint** [คู่มือ](https://example.com)\n```rs\nsecret()\n```\nลอง `command` https://example.com"
            ),
            "สวัสดี Mint คู่มือ ลอง"
        );
    }
    #[tokio::test]
    async fn an_enabled_companion_owns_automatic_reading_until_its_route_expires() {
        let path =
            std::env::temp_dir().join(format!("mint-speech-route-{}.sqlite", uuid::Uuid::new_v4()));
        let memory = MemoryStore::open(&path);
        let route = uuid::Uuid::new_v4().to_string();
        let playback = uuid::Uuid::new_v4().to_string();
        handle(
            &memory,
            SpeechRequest::Prefer {
                owner: route.clone(),
                chat_id: "chat".into(),
                enabled: true,
            },
        )
        .await
        .unwrap();
        assert!(
            acquire(&memory, &playback, "chat", Some(1), None)
                .unwrap_err()
                .contains("routed")
        );
        acquire(&memory, &playback, "chat", Some(1), Some(&route)).unwrap();
        cancel_chat(&memory, "chat");
        handle(
            &memory,
            SpeechRequest::Release {
                owner: playback.clone(),
            },
        )
        .await
        .unwrap();
        db(&memory)
            .unwrap()
            .execute("UPDATE speech_routes SET expires=?1", [now() - 1])
            .unwrap();
        acquire(
            &memory,
            &uuid::Uuid::new_v4().to_string(),
            "chat",
            Some(2),
            None,
        )
        .unwrap();
        drop(memory);
        let _ = std::fs::remove_file(path);
    }
    #[tokio::test]
    async fn cancelled_preparation_and_stale_release_cannot_take_over_another_run() {
        let path = std::env::temp_dir().join(format!(
            "mint-speech-cancel-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let memory = MemoryStore::open(&path);
        let first = uuid::Uuid::new_v4().to_string();
        let second = uuid::Uuid::new_v4().to_string();
        handle(
            &memory,
            SpeechRequest::Release {
                owner: first.clone(),
            },
        )
        .await
        .unwrap();
        assert!(acquire(&memory, &first, "chat", None, None).is_err());
        acquire(&memory, &second, "chat", None, None).unwrap();
        handle(
            &memory,
            SpeechRequest::Release {
                owner: first.clone(),
            },
        )
        .await
        .unwrap();
        assert!(active(&memory, &second).unwrap());
        db(&memory)
            .unwrap()
            .execute("UPDATE speech_lease SET expires=?1", [now() - 1])
            .unwrap();
        assert!(!active(&memory, &second).unwrap());
        let third = uuid::Uuid::new_v4().to_string();
        acquire(&memory, &third, "chat", None, None).unwrap();
        assert!(active(&memory, &third).unwrap());
        drop(memory);
        let _ = std::fs::remove_file(path);
    }
    #[tokio::test]
    async fn unfinished_turns_and_code_only_replies_never_request_provider_audio() {
        let path =
            std::env::temp_dir().join(format!("mint-speech-turn-{}.sqlite", uuid::Uuid::new_v4()));
        let memory = MemoryStore::open(&path);
        let id = memory.start_turn("chat", "hello").unwrap();
        let request = |id| SpeechRequest::Prepare {
            owner: uuid::Uuid::new_v4().to_string(),
            chat_id: "chat".into(),
            turn_id: Some(id),
            text: None,
            automatic: false,
            audio_only: true,
            route_owner: None,
        };
        assert!(
            handle(&memory, request(id))
                .await
                .unwrap_err()
                .contains("completed")
        );
        let id = memory
            .add_interaction_for_chat("chat", "code", "```rs\nsecret();\n```", "test", "test")
            .unwrap();
        assert!(
            handle(&memory, request(id))
                .await
                .unwrap_err()
                .contains("readable")
        );
        let older = memory
            .add_interaction_for_chat("chat", "one", "Older answer", "test", "test")
            .unwrap();
        memory
            .add_interaction_for_chat("chat", "two", "Newer answer", "test", "test")
            .unwrap();
        let mut automatic = request(older);
        if let SpeechRequest::Prepare { automatic, .. } = &mut automatic {
            *automatic = true;
        }
        assert!(
            handle(&memory, automatic)
                .await
                .unwrap_err()
                .contains("newer")
        );
        drop(memory);
        let _ = std::fs::remove_file(path);
    }
    #[tokio::test]
    async fn playback_owner_is_exclusive_across_connections_and_auto_replies_are_deduplicated() {
        let path =
            std::env::temp_dir().join(format!("mint-speech-{}.sqlite", uuid::Uuid::new_v4()));
        let a = MemoryStore::open(&path);
        let b = MemoryStore::open(&path);
        let first = uuid::Uuid::new_v4().to_string();
        let second = uuid::Uuid::new_v4().to_string();
        acquire(&a, &first, "chat", Some(1), None).unwrap();
        assert!(acquire(&b, &second, "chat", Some(1), None).is_err());
        cancel_chat(&a, "chat");
        assert!(!active(&b, &first).unwrap());
        assert!(
            acquire(&b, &second, "chat", None, None).is_err(),
            "revocation must wait for the previous player to acknowledge stopping"
        );
        handle(
            &a,
            SpeechRequest::Release {
                owner: first.clone(),
            },
        )
        .await
        .unwrap();
        assert!(acquire(&b, &second, "chat", Some(1), None).is_err());
        acquire(&b, &second, "chat", None, None).unwrap();
        assert!(active(&a, &second).unwrap());
        drop(a);
        drop(b);
        let _ = std::fs::remove_file(path);
    }
}

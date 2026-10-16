use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostDescriptor {
    pub version: u32,
    pub host_id: String,
    pub name: String,
    pub endpoint: String,
    pub token: String,
}

pub fn hosts_dir() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|p| p.join("mint/companion/hosts"))
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Queued,
    #[default]
    Thinking,
    Working,
    Responding,
    Waiting,
    Completed,
    Failed,
    Interrupted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub chat_id: String,
    pub title: String,
    pub workspace: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Turn {
    pub chat_id: String,
    pub turn_id: i64,
    pub seq: u64,
    pub status: Status,
    pub tool: Option<String>,
    pub text: String,
    #[serde(default)]
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    ListSessions,
    Subscribe {
        #[serde(rename = "chatId")]
        chat_id: String,
    },
    SendChat {
        #[serde(rename = "chatId")]
        chat_id: String,
        #[serde(rename = "requestId")]
        request_id: String,
        message: String,
    },
    Interact {
        #[serde(rename = "chatId")]
        chat_id: String,
        #[serde(rename = "requestId")]
        request_id: String,
        area: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    Snapshot {
        #[serde(rename = "hostId")]
        host_id: String,
        version: u32,
        sessions: Vec<Session>,
        turns: Vec<Turn>,
    },
    Update {
        #[serde(rename = "hostId")]
        host_id: String,
        turn: Turn,
    },
    Accepted {
        #[serde(rename = "requestId")]
        request_id: String,
        #[serde(rename = "turnId")]
        turn_id: Option<i64>,
    },
    Error {
        #[serde(rename = "requestId")]
        request_id: Option<String>,
        message: String,
    },
}

#[derive(Default)]
pub struct TurnBook(pub BTreeMap<(String, i64), Turn>);

impl TurnBook {
    pub fn apply(&mut self, turn: Turn) -> bool {
        let key = (turn.chat_id.clone(), turn.turn_id);
        if self.0.get(&key).is_some_and(|old| old.seq >= turn.seq) {
            return false;
        }
        self.0.insert(key, turn);
        true
    }
    pub fn session(&self, chat_id: &str) -> Vec<Turn> {
        self.0
            .values()
            .filter(|t| t.chat_id == chat_id)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn turn(chat: &str, id: i64, seq: u64, status: Status) -> Turn {
        Turn {
            chat_id: chat.into(),
            turn_id: id,
            seq,
            status,
            tool: None,
            text: String::new(),
            prompt: String::new(),
        }
    }
    #[test]
    fn snapshots_keep_concurrent_sessions_separate() {
        let mut book = TurnBook::default();
        book.apply(turn("one", 1, 1, Status::Working));
        book.apply(turn("two", 2, 2, Status::Responding));
        assert_eq!(book.session("one").len(), 1);
        assert_eq!(book.session("one")[0].status, Status::Working);
    }
    #[test]
    fn late_updates_cannot_overwrite_completed_turns() {
        let mut book = TurnBook::default();
        assert!(book.apply(turn("one", 1, 3, Status::Completed)));
        assert!(!book.apply(turn("one", 1, 2, Status::Thinking)));
        assert_eq!(book.session("one")[0].status, Status::Completed);
    }
    #[test]
    fn clients_cannot_inject_tools_or_system_instructions() {
        let wire =
            r#"{"type":"send_chat","chatId":"one","requestId":"req","message":"hi","tools":[{}]}"#;
        assert!(serde_json::from_str::<Command>(wire).is_err());
    }
}

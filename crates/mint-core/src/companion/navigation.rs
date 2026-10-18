//! Host-owned conversation handoff. Clients supply only a persisted chat ID.
use mint_companion_protocol::Session;
pub use mint_companion_protocol::{NavigationOutcome, Surface};
use std::sync::{Arc, Mutex};

type Handler = Arc<dyn Fn(Session, bool) -> Result<NavigationOutcome, String> + Send + Sync>;
static DESTINATION: Mutex<Option<(Surface, Handler)>> = Mutex::new(None);

pub fn configure(
    surface: Surface,
    handler: impl Fn(Session, bool) -> Result<NavigationOutcome, String> + Send + Sync + 'static,
) {
    *DESTINATION.lock().unwrap() = Some((surface, Arc::new(handler)));
}

pub fn surface() -> Option<Surface> {
    DESTINATION
        .lock()
        .unwrap()
        .as_ref()
        .map(|(surface, _)| *surface)
}

pub async fn open(session: Session, new_terminal: bool) -> Result<NavigationOutcome, String> {
    let handler = DESTINATION
        .lock()
        .unwrap()
        .as_ref()
        .map(|(_, handler)| Arc::clone(handler))
        .ok_or("This Mint host does not support opening conversations. Update Mint.")?;
    tokio::task::spawn_blocking(move || handler(session, new_terminal))
        .await
        .map_err(|error| error.to_string())?
}

pub fn conversation_url(base: &str, chat_id: &str) -> Result<String, String> {
    let mut url = reqwest::Url::parse(base).map_err(|error| error.to_string())?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Invalid Mint Web address".into());
    }
    url.set_fragment(None);
    let retained: Vec<_> = url
        .query_pairs()
        .filter(|(key, _)| key != "id")
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    url.set_query(None);
    url.query_pairs_mut()
        .extend_pairs(retained)
        .append_pair("id", chat_id);
    Ok(url.into())
}

pub fn configure_web(base: String) {
    configure(Surface::Web, move |session, _| {
        let url = conversation_url(&base, &session.chat_id)?;
        #[cfg(target_os = "linux")]
        let mut command = {
            let mut c = std::process::Command::new("xdg-open");
            c.arg(&url);
            c
        };
        #[cfg(target_os = "macos")]
        let mut command = {
            let mut c = std::process::Command::new("open");
            c.arg(&url);
            c
        };
        #[cfg(target_os = "windows")]
        let mut command = {
            let mut c = std::process::Command::new("rundll32");
            c.args(["url.dll,FileProtocolHandler", &url]);
            c
        };
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        return Err("Opening Mint Web is unsupported on this platform".into());
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            let status = command
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map_err(|error| error.to_string())?;
            if !status.success() {
                return Err("Could not open Mint Web in your browser".into());
            }
            Ok(NavigationOutcome::Opened)
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn web_handoff_preserves_origin_and_encodes_the_exact_conversation() {
        let url = conversation_url(
            "http://localhost:9123/?id=old&theme=dark",
            "cli::a & ไทย/#?",
        )
        .unwrap();
        let url = reqwest::Url::parse(&url).unwrap();
        assert_eq!(url.port(), Some(9123));
        assert_eq!(
            url.query_pairs().find(|(k, _)| k == "id").unwrap().1,
            "cli::a & ไทย/#?"
        );
        assert_eq!(url.query_pairs().filter(|(k, _)| k == "id").count(), 1);
        assert!(conversation_url("file:///tmp/test", "chat").is_err());
    }
}

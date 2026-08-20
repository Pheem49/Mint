//! Shared Mint identity store. Every Mint app on this machine (Mint search,
//! mint agent, ...) reads and writes the same SQLite file so an account
//! created in one app works in all of them.

use std::path::PathBuf;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("unable to determine the user config directory")]
    ConfigDirectoryUnavailable,
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("email and password are required")]
    MissingCredentials,
    #[error("password must be at least 8 characters")]
    PasswordTooShort,
    #[error("an account with this email already exists")]
    EmailTaken,
    #[error("invalid email or password")]
    InvalidCredentials,
    #[error("password hashing failed: {0}")]
    Hash(#[from] bcrypt::BcryptError),
    #[error("unable to save avatar file: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthUser {
    pub id: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub image: Option<String>,
}

/// Path to the shared identity database: `~/.config/mint/mint-user.sqlite`.
pub fn user_db_path() -> Result<PathBuf, AuthError> {
    dirs::config_dir()
        .map(|dir| dir.join("mint").join("mint-user.sqlite"))
        .ok_or(AuthError::ConfigDirectoryUnavailable)
}

/// Path to the shared profile pictures directory:
/// `~/.config/mint/Pictures/profile`.
pub fn profile_pictures_dir() -> Result<PathBuf, AuthError> {
    let dir = dirs::config_dir()
        .map(|dir| dir.join("mint").join("Pictures").join("profile"))
        .ok_or(AuthError::ConfigDirectoryUnavailable)?;
    std::fs::create_dir_all(&dir).ok();
    Ok(dir)
}

/// Saves an avatar image into the shared profile pictures directory and
/// returns the `/api/avatar?key=...` URL to store on the user record.
/// `extension` should be a bare extension like "jpg" or "png" (no dot).
pub fn save_avatar_file(bytes: &[u8], extension: &str) -> Result<String, AuthError> {
    let dir = profile_pictures_dir()?;
    let extension = if extension.is_empty() {
        "png"
    } else {
        extension
    };
    let file_name = format!(
        "profile_{}.{}",
        chrono::Utc::now().timestamp_millis(),
        extension
    );
    std::fs::write(dir.join(&file_name), bytes)?;
    Ok(format!("/api/avatar?key={file_name}"))
}

const SCHEMA_SQL: &str = "CREATE TABLE IF NOT EXISTS user (
            id TEXT PRIMARY KEY,
            name TEXT,
            email TEXT UNIQUE,
            emailVerified INTEGER,
            image TEXT,
            passwordHash TEXT
        );
        CREATE TABLE IF NOT EXISTS account (
            userId TEXT NOT NULL,
            type TEXT NOT NULL,
            provider TEXT NOT NULL,
            providerAccountId TEXT NOT NULL,
            refresh_token TEXT,
            access_token TEXT,
            expires_at INTEGER,
            token_type TEXT,
            scope TEXT,
            id_token TEXT,
            session_state TEXT,
            PRIMARY KEY (provider, providerAccountId)
        );
        CREATE TABLE IF NOT EXISTS session (
            sessionToken TEXT PRIMARY KEY,
            userId TEXT NOT NULL,
            expires INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS verificationToken (
            identifier TEXT NOT NULL,
            token TEXT NOT NULL,
            expires INTEGER NOT NULL,
            PRIMARY KEY (identifier, token)
        );";

fn init_schema(conn: &Connection) -> Result<(), AuthError> {
    conn.execute_batch(SCHEMA_SQL)?;
    Ok(())
}

fn open_user_db() -> Result<Connection, AuthError> {
    let path = user_db_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let conn = Connection::open(path)?;
    init_schema(&conn)?;
    Ok(conn)
}

pub fn register_user(
    name: Option<String>,
    email: &str,
    password: &str,
) -> Result<AuthUser, AuthError> {
    register_user_with_conn(&open_user_db()?, name, email, password)
}

/// Core registration logic, decoupled from the real on-disk database so it
/// can run against an isolated connection (e.g. a temp-file SQLite db) in
/// tests. `register_user` is the production entry point; call this directly
/// only from tests.
fn register_user_with_conn(
    conn: &Connection,
    name: Option<String>,
    email: &str,
    password: &str,
) -> Result<AuthUser, AuthError> {
    let email = email.trim();
    if email.is_empty() || password.is_empty() {
        return Err(AuthError::MissingCredentials);
    }
    if password.len() < 8 {
        return Err(AuthError::PasswordTooShort);
    }

    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM user WHERE email = ?1",
            params![email],
            |row| row.get(0),
        )
        .optional()?;
    if existing.is_some() {
        return Err(AuthError::EmailTaken);
    }

    let id = Uuid::new_v4().to_string();
    let password_hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)?;
    let display_name = name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| email.split('@').next().unwrap_or(email).to_string());

    conn.execute(
        "INSERT INTO user (id, name, email, passwordHash) VALUES (?1, ?2, ?3, ?4)",
        params![id, display_name, email, password_hash],
    )?;

    Ok(AuthUser {
        id,
        name: Some(display_name),
        email: Some(email.to_string()),
        image: None,
    })
}

pub fn login_user(email: &str, password: &str) -> Result<AuthUser, AuthError> {
    login_user_with_conn(&open_user_db()?, email, password)
}

fn login_user_with_conn(
    conn: &Connection,
    email: &str,
    password: &str,
) -> Result<AuthUser, AuthError> {
    let email = email.trim();
    if email.is_empty() || password.is_empty() {
        return Err(AuthError::MissingCredentials);
    }

    let row: Option<(
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = conn
        .query_row(
            "SELECT id, name, email, image, passwordHash FROM user WHERE email = ?1",
            params![email],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()?;

    let Some((id, name, email, image, password_hash)) = row else {
        return Err(AuthError::InvalidCredentials);
    };
    let Some(hash) = password_hash else {
        return Err(AuthError::InvalidCredentials);
    };
    if !bcrypt::verify(password, &hash)? {
        return Err(AuthError::InvalidCredentials);
    }

    Ok(AuthUser {
        id,
        name,
        email,
        image,
    })
}

pub fn get_user(id: &str) -> Result<Option<AuthUser>, AuthError> {
    get_user_with_conn(&open_user_db()?, id)
}

fn get_user_with_conn(conn: &Connection, id: &str) -> Result<Option<AuthUser>, AuthError> {
    let user = conn
        .query_row(
            "SELECT id, name, email, image FROM user WHERE id = ?1",
            params![id],
            |row| {
                Ok(AuthUser {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    email: row.get(2)?,
                    image: row.get(3)?,
                })
            },
        )
        .optional()?;
    Ok(user)
}

pub fn update_profile(
    id: &str,
    name: Option<String>,
    image: Option<String>,
) -> Result<AuthUser, AuthError> {
    update_profile_with_conn(&open_user_db()?, id, name, image)
}

fn update_profile_with_conn(
    conn: &Connection,
    id: &str,
    name: Option<String>,
    image: Option<String>,
) -> Result<AuthUser, AuthError> {
    if let Some(name) = &name {
        conn.execute("UPDATE user SET name = ?1 WHERE id = ?2", params![name, id])?;
    }
    if let Some(image) = &image {
        conn.execute(
            "UPDATE user SET image = ?1 WHERE id = ?2",
            params![image, id],
        )?;
    }
    get_user_with_conn(conn, id)?.ok_or(AuthError::InvalidCredentials)
}

// --- Session tokens (used by the web-mode API server; the desktop app uses
// its own persisted "current user id" file instead, see desktop.rs).
//
// Persisted in the shared `session` table (not just in memory) so a
// browser's saved token still works after the API server process restarts
// — otherwise every restart would silently log everyone out. ---

const SESSION_LIFETIME_DAYS: i64 = 30;

pub fn create_session(user_id: &str) -> String {
    let Ok(conn) = open_user_db() else {
        return Uuid::new_v4().to_string();
    };
    create_session_with_conn(&conn, user_id)
}

fn create_session_with_conn(conn: &Connection, user_id: &str) -> String {
    let token = Uuid::new_v4().to_string();
    let expires =
        (chrono::Utc::now() + chrono::Duration::days(SESSION_LIFETIME_DAYS)).timestamp_millis();
    let _ = conn.execute(
        "INSERT INTO session (sessionToken, userId, expires) VALUES (?1, ?2, ?3)",
        params![token, user_id, expires],
    );
    token
}

pub fn session_user_id(token: &str) -> Option<String> {
    session_user_id_with_conn(&open_user_db().ok()?, token)
}

fn session_user_id_with_conn(conn: &Connection, token: &str) -> Option<String> {
    let now = chrono::Utc::now().timestamp_millis();
    conn.query_row(
        "SELECT userId FROM session WHERE sessionToken = ?1 AND expires > ?2",
        params![token, now],
        |row| row.get(0),
    )
    .optional()
    .ok()
    .flatten()
}

pub fn destroy_session(token: &str) {
    if let Ok(conn) = open_user_db() {
        destroy_session_with_conn(&conn, token);
    }
}

fn destroy_session_with_conn(conn: &Connection, token: &str) {
    let _ = conn.execute(
        "DELETE FROM session WHERE sessionToken = ?1",
        params![token],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// A real (temp-file, not in-memory) SQLite connection with the schema
    /// applied — mirrors `open_user_db()` but points at a throwaway file
    /// instead of the real `~/.config/mint/mint-user.sqlite`, so these tests
    /// can never touch a real user's account data. Each test gets a unique
    /// path (pid + nanos) so `cargo test`'s parallel test threads don't
    /// collide, matching the pattern used in tests/memory_persistence.rs.
    fn test_conn(name: &str) -> Connection {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "mint-core-auth-test-{name}-{}-{nanos}.sqlite",
            std::process::id(),
        ));
        let _ = std::fs::remove_file(&path);
        let conn = Connection::open(&path).expect("open temp sqlite db");
        init_schema(&conn).expect("init schema");
        conn
    }

    // -- register_user ---------------------------------------------------

    #[test]
    fn register_user_succeeds_and_hashes_password() {
        let conn = test_conn("register-ok");
        let user = register_user_with_conn(
            &conn,
            Some("Pheem".to_string()),
            "pheem@example.com",
            "hunter22",
        )
        .expect("registration should succeed");

        assert_eq!(user.email.as_deref(), Some("pheem@example.com"));
        assert_eq!(user.name.as_deref(), Some("Pheem"));

        let stored_hash: String = conn
            .query_row(
                "SELECT passwordHash FROM user WHERE id = ?1",
                params![user.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_ne!(
            stored_hash, "hunter22",
            "password must not be stored in plaintext"
        );
        assert!(bcrypt::verify("hunter22", &stored_hash).unwrap());
    }

    #[test]
    fn register_user_defaults_name_from_email_when_blank() {
        let conn = test_conn("register-default-name");
        let user = register_user_with_conn(&conn, None, "nickname@example.com", "hunter22")
            .expect("registration should succeed");
        assert_eq!(user.name.as_deref(), Some("nickname"));
    }

    #[test]
    fn register_user_rejects_duplicate_email() {
        let conn = test_conn("register-dup");
        register_user_with_conn(&conn, None, "dup@example.com", "hunter22").unwrap();
        let result = register_user_with_conn(&conn, None, "dup@example.com", "different1");
        assert!(matches!(result, Err(AuthError::EmailTaken)));
    }

    #[test]
    fn register_user_rejects_short_password() {
        let conn = test_conn("register-short-pw");
        let result = register_user_with_conn(&conn, None, "short@example.com", "abc123");
        assert!(matches!(result, Err(AuthError::PasswordTooShort)));
    }

    #[test]
    fn register_user_rejects_empty_email_or_password() {
        let conn = test_conn("register-empty");
        assert!(matches!(
            register_user_with_conn(&conn, None, "", "hunter22"),
            Err(AuthError::MissingCredentials)
        ));
        assert!(matches!(
            register_user_with_conn(&conn, None, "someone@example.com", ""),
            Err(AuthError::MissingCredentials)
        ));
    }

    // -- login_user -------------------------------------------------------

    #[test]
    fn login_user_succeeds_with_correct_password() {
        let conn = test_conn("login-ok");
        register_user_with_conn(&conn, None, "login@example.com", "correct-horse").unwrap();
        let user = login_user_with_conn(&conn, "login@example.com", "correct-horse")
            .expect("login should succeed");
        assert_eq!(user.email.as_deref(), Some("login@example.com"));
    }

    #[test]
    fn login_user_rejects_wrong_password() {
        let conn = test_conn("login-wrong-pw");
        register_user_with_conn(&conn, None, "login2@example.com", "correct-horse").unwrap();
        let result = login_user_with_conn(&conn, "login2@example.com", "wrong-password");
        assert!(matches!(result, Err(AuthError::InvalidCredentials)));
    }

    #[test]
    fn login_user_rejects_unknown_email() {
        let conn = test_conn("login-unknown");
        let result = login_user_with_conn(&conn, "ghost@example.com", "whatever1");
        assert!(matches!(result, Err(AuthError::InvalidCredentials)));
    }

    #[test]
    fn login_user_rejects_empty_credentials() {
        let conn = test_conn("login-empty");
        assert!(matches!(
            login_user_with_conn(&conn, "", "whatever1"),
            Err(AuthError::MissingCredentials)
        ));
    }

    // -- get_user / update_profile ----------------------------------------

    #[test]
    fn get_user_round_trips_registered_user() {
        let conn = test_conn("get-user");
        let created =
            register_user_with_conn(&conn, Some("Name".into()), "getme@example.com", "hunter22")
                .unwrap();
        let fetched = get_user_with_conn(&conn, &created.id).unwrap();
        assert_eq!(
            fetched.map(|u| u.email),
            Some(Some("getme@example.com".to_string()))
        );
    }

    #[test]
    fn get_user_returns_none_for_unknown_id() {
        let conn = test_conn("get-user-missing");
        assert!(get_user_with_conn(&conn, "no-such-id").unwrap().is_none());
    }

    #[test]
    fn update_profile_changes_name_and_image() {
        let conn = test_conn("update-profile");
        let created =
            register_user_with_conn(&conn, None, "update@example.com", "hunter22").unwrap();
        let updated = update_profile_with_conn(
            &conn,
            &created.id,
            Some("New Name".to_string()),
            Some("/api/avatar?key=x.png".to_string()),
        )
        .unwrap();
        assert_eq!(updated.name.as_deref(), Some("New Name"));
        assert_eq!(updated.image.as_deref(), Some("/api/avatar?key=x.png"));
    }

    // -- sessions -----------------------------------------------------------

    #[test]
    fn session_create_then_resolve_returns_user_id() {
        let conn = test_conn("session-resolve");
        let user = register_user_with_conn(&conn, None, "session@example.com", "hunter22").unwrap();
        let token = create_session_with_conn(&conn, &user.id);
        assert_eq!(
            session_user_id_with_conn(&conn, &token).as_deref(),
            Some(user.id.as_str())
        );
    }

    #[test]
    fn session_unknown_token_resolves_to_none() {
        let conn = test_conn("session-unknown");
        assert_eq!(session_user_id_with_conn(&conn, "no-such-token"), None);
    }

    #[test]
    fn session_destroy_makes_it_unresolvable() {
        let conn = test_conn("session-destroy");
        let user = register_user_with_conn(&conn, None, "destroy@example.com", "hunter22").unwrap();
        let token = create_session_with_conn(&conn, &user.id);
        destroy_session_with_conn(&conn, &token);
        assert_eq!(session_user_id_with_conn(&conn, &token), None);
    }

    #[test]
    fn session_expired_resolves_to_none() {
        let conn = test_conn("session-expired");
        let user = register_user_with_conn(&conn, None, "expired@example.com", "hunter22").unwrap();
        let expired_token = Uuid::new_v4().to_string();
        let already_expired = (chrono::Utc::now() - chrono::Duration::days(1)).timestamp_millis();
        conn.execute(
            "INSERT INTO session (sessionToken, userId, expires) VALUES (?1, ?2, ?3)",
            params![expired_token, user.id, already_expired],
        )
        .unwrap();
        assert_eq!(session_user_id_with_conn(&conn, &expired_token), None);
    }
}

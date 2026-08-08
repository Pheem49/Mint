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

fn open_user_db() -> Result<Connection, AuthError> {
    let path = user_db_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS user (
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
        );",
    )?;
    Ok(conn)
}

pub fn register_user(
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

    let conn = open_user_db()?;

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
    let email = email.trim();
    if email.is_empty() || password.is_empty() {
        return Err(AuthError::MissingCredentials);
    }

    let conn = open_user_db()?;
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
    let conn = open_user_db()?;
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
    let conn = open_user_db()?;
    if let Some(name) = &name {
        conn.execute("UPDATE user SET name = ?1 WHERE id = ?2", params![name, id])?;
    }
    if let Some(image) = &image {
        conn.execute(
            "UPDATE user SET image = ?1 WHERE id = ?2",
            params![image, id],
        )?;
    }
    get_user(id)?.ok_or(AuthError::InvalidCredentials)
}

// --- Session tokens (used by the web-mode API server; the desktop app uses
// its own persisted "current user id" file instead, see desktop.rs).
//
// Persisted in the shared `session` table (not just in memory) so a
// browser's saved token still works after the API server process restarts
// — otherwise every restart would silently log everyone out. ---

const SESSION_LIFETIME_DAYS: i64 = 30;

pub fn create_session(user_id: &str) -> String {
    let token = Uuid::new_v4().to_string();
    if let Ok(conn) = open_user_db() {
        let expires =
            (chrono::Utc::now() + chrono::Duration::days(SESSION_LIFETIME_DAYS)).timestamp_millis();
        let _ = conn.execute(
            "INSERT INTO session (sessionToken, userId, expires) VALUES (?1, ?2, ?3)",
            params![token, user_id, expires],
        );
    }
    token
}

pub fn session_user_id(token: &str) -> Option<String> {
    let conn = open_user_db().ok()?;
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
        let _ = conn.execute(
            "DELETE FROM session WHERE sessionToken = ?1",
            params![token],
        );
    }
}

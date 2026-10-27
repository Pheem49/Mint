use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const MAX_IMAGE_SIZE: usize = 25 * 1024 * 1024; // 25 MB
pub const MAX_CACHE_DIR_SIZE: u64 = 200 * 1024 * 1024; // 200 MB
pub const PRUNE_DOWN_TO_SIZE: u64 = 150 * 1024 * 1024; // 150 MB
pub const CACHE_TTL_SECS: u64 = 7 * 86400; // 7 days

#[derive(Debug, Error)]
pub enum ImageProxyError {
    #[error("invalid url: {0}")]
    InvalidUrl(String),
    #[error("ssrf blocked: {0}")]
    SsrfBlocked(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("target returned bad status: {0}")]
    BadStatus(u16),
    #[error("target content type is not an image: {0}")]
    NotAnImage(String),
    #[error("image exceeds max size of {0} bytes")]
    TooLarge(usize),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CacheMeta {
    pub url: String,
    pub content_type: String,
    pub created_at: u64,
    pub size: usize,
}

/// Computes a stable SHA-256 cache key for an image URL.
pub fn url_cache_key(url: &str) -> String {
    format!("{:x}", Sha256::digest(url.as_bytes()))
}

/// Directory path for disk-based image caching (~/.config/mint/cache/image-proxy).
pub fn default_cache_dir() -> PathBuf {
    crate::system::config::mint_config_dir()
        .map(|d| d.join("cache").join("image-proxy"))
        .unwrap_or_else(|| std::env::temp_dir().join("mint-image-proxy"))
}

/// Legacy directory path for image proxy cache (~/.config/mint/Pictures/image-cache).
pub(crate) fn legacy_cache_dir() -> Option<PathBuf> {
    crate::system::config::mint_config_dir().map(|d| d.join("Pictures").join("image-cache"))
}

/// Internal helper that removes a legacy cache directory path if it exists on disk.
pub(crate) fn cleanup_legacy_cache_dir_in(legacy_path: Option<&Path>) -> bool {
    if let Some(path) = legacy_path {
        if path.is_dir() {
            return fs::remove_dir_all(path).is_ok();
        }
    }
    false
}

/// Automatically removes legacy cache directory (~/.config/mint/Pictures/image-cache)
/// if it exists, preventing orphaned disk consumption for upgraded users.
pub(crate) fn cleanup_legacy_cache_dir() -> bool {
    cleanup_legacy_cache_dir_in(legacy_cache_dir().as_deref())
}

static CLEANUP_ONCE: std::sync::Once = std::sync::Once::new();

/// Proactively ensures legacy cache directory is cleaned up once per process lifetime.
pub(crate) fn ensure_legacy_cache_cleaned() {
    CLEANUP_ONCE.call_once(|| {
        let _ = cleanup_legacy_cache_dir();
    });
}

/// Returns true if an IP address is private, loopback, link-local, or broadcast.
pub fn is_private_or_loopback_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.octets()[0] == 0
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_multicast()
                || ((v6.segments()[0] & 0xfe00) == 0xfc00) // fc00::/7
                || ((v6.segments()[0] & 0xffc0) == 0xfe80) // fe80::/10
        }
    }
}

/// Validates that a target URL is safe to fetch via proxy (SSRF guard).
pub async fn validate_target_url(target_url: &str) -> Result<reqwest::Url, ImageProxyError> {
    let parsed =
        reqwest::Url::parse(target_url).map_err(|e| ImageProxyError::InvalidUrl(e.to_string()))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(ImageProxyError::InvalidUrl(
            "only http and https schemes are permitted".into(),
        ));
    }

    let host = parsed
        .host_str()
        .ok_or_else(|| ImageProxyError::InvalidUrl("missing host in url".into()))?;

    if host.eq_ignore_ascii_case("localhost")
        || host.ends_with(".localhost")
        || host.ends_with(".local")
    {
        return Err(ImageProxyError::SsrfBlocked(
            "localhost and .local domains are blocked".into(),
        ));
    }

    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_private_or_loopback_ip(ip) {
            return Err(ImageProxyError::SsrfBlocked(format!(
                "IP {ip} is private/loopback"
            )));
        }
    }

    let port = parsed.port_or_known_default().unwrap_or(80);
    if let Ok(addrs) = tokio::net::lookup_host(format!("{host}:{port}")).await {
        for addr in addrs {
            if is_private_or_loopback_ip(addr.ip()) {
                return Err(ImageProxyError::SsrfBlocked(format!(
                    "host {host} resolves to private IP {}",
                    addr.ip()
                )));
            }
        }
    }

    Ok(parsed)
}

/// Attempts to read an image and its content-type from the disk cache.
pub fn read_from_cache(cache_dir: &Path, key: &str) -> Option<(String, Vec<u8>)> {
    let bin_path = cache_dir.join(format!("{key}.bin"));
    let meta_path = cache_dir.join(format!("{key}.meta.json"));

    if !bin_path.exists() || !meta_path.exists() {
        return None;
    }

    let meta_str = fs::read_to_string(&meta_path).ok()?;
    let meta: CacheMeta = serde_json::from_str(&meta_str).ok()?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    if now.saturating_sub(meta.created_at) > CACHE_TTL_SECS {
        // Expired
        let _ = fs::remove_file(bin_path);
        let _ = fs::remove_file(meta_path);
        return None;
    }

    let bytes = fs::read(bin_path).ok()?;
    Some((meta.content_type, bytes))
}

/// Writes fetched image bytes and metadata into the disk cache.
pub fn write_to_cache(
    cache_dir: &Path,
    key: &str,
    url: &str,
    content_type: &str,
    bytes: &[u8],
) -> Result<(), std::io::Error> {
    fs::create_dir_all(cache_dir)?;
    let bin_path = cache_dir.join(format!("{key}.bin"));
    let meta_path = cache_dir.join(format!("{key}.meta.json"));

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let meta = CacheMeta {
        url: url.to_owned(),
        content_type: content_type.to_owned(),
        created_at: now,
        size: bytes.len(),
    };

    fs::write(&bin_path, bytes)?;
    if let Ok(meta_json) = serde_json::to_string(&meta) {
        let _ = fs::write(&meta_path, meta_json);
    }

    let _ = prune_cache_if_needed(cache_dir);
    Ok(())
}

/// Prunes oldest cache entries if total directory size exceeds `max_bytes` down to `target_bytes`.
/// Returns the number of bytes reclaimed.
pub fn prune_cache_to_size(
    cache_dir: &Path,
    max_bytes: u64,
    target_bytes: u64,
) -> Result<u64, std::io::Error> {
    let entries = fs::read_dir(cache_dir)?;
    let mut files = Vec::new();
    let mut total_size = 0u64;

    for entry in entries.flatten() {
        if let Ok(metadata) = entry.metadata() {
            if metadata.is_file() {
                let size = metadata.len();
                total_size += size;
                let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                files.push((entry.path(), size, modified));
            }
        }
    }

    if total_size <= max_bytes {
        return Ok(0);
    }

    let initial_size = total_size;
    // Sort oldest first
    files.sort_by_key(|(_, _, modified)| *modified);

    for (path, size, _) in files {
        if total_size <= target_bytes {
            break;
        }
        if fs::remove_file(&path).is_ok() {
            total_size = total_size.saturating_sub(size);
        }
    }

    Ok(initial_size.saturating_sub(total_size))
}

/// Prunes oldest cache entries if total directory size exceeds `MAX_CACHE_DIR_SIZE`
/// and cleans up any legacy orphaned cache folder.
pub fn prune_cache_if_needed(cache_dir: &Path) -> Result<(), std::io::Error> {
    ensure_legacy_cache_cleaned();
    let _ = prune_cache_to_size(cache_dir, MAX_CACHE_DIR_SIZE, PRUNE_DOWN_TO_SIZE)?;
    Ok(())
}

/// Fetches a remote image safely, verifying content-type and size, and writes through cache.
pub async fn fetch_and_cache_image(
    cache_dir: &Path,
    target_url: &str,
) -> Result<(String, Vec<u8>), ImageProxyError> {
    ensure_legacy_cache_cleaned();
    let key = url_cache_key(target_url);

    // 1. Check local cache
    if let Some(cached) = read_from_cache(cache_dir, &key) {
        return Ok(cached);
    }

    // 2. Validate URL & SSRF checks
    let parsed_url = validate_target_url(target_url).await?;

    // 3. Perform HTTP request
    let client = crate::HTTP_CLIENT.clone();
    let response = client
        .get(parsed_url)
        .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
        .header("Accept", "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8")
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| ImageProxyError::Network(e.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        return Err(ImageProxyError::BadStatus(status.as_u16()));
    }

    // 4. Validate Content-Type
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_owned();

    let is_image_mime = content_type.starts_with("image/")
        || content_type.contains("octet-stream")
        || content_type.contains("webp");

    if !is_image_mime {
        return Err(ImageProxyError::NotAnImage(content_type));
    }

    // 5. Read body with size ceiling
    if let Some(cl) = response.content_length() {
        if cl as usize > MAX_IMAGE_SIZE {
            return Err(ImageProxyError::TooLarge(cl as usize));
        }
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| ImageProxyError::Network(e.to_string()))?;

    if bytes.len() > MAX_IMAGE_SIZE {
        return Err(ImageProxyError::TooLarge(bytes.len()));
    }

    // Derive effective content type if octet-stream
    let effective_type = if content_type.contains("octet-stream") {
        if bytes.starts_with(b"\x89PNG") {
            "image/png".to_owned()
        } else if bytes.starts_with(b"\xff\xd8\xff") {
            "image/jpeg".to_owned()
        } else if bytes.starts_with(b"RIFF") && bytes.len() > 12 && &bytes[8..12] == b"WEBP" {
            "image/webp".to_owned()
        } else if bytes.starts_with(b"GIF8") {
            "image/gif".to_owned()
        } else {
            content_type
        }
    } else {
        content_type
    };

    // 6. Cache on disk
    let _ = write_to_cache(cache_dir, &key, target_url, &effective_type, &bytes);

    Ok((effective_type, bytes.to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_validate_target_url_blocks_localhost() {
        assert!(
            validate_target_url("http://localhost/image.png")
                .await
                .is_err()
        );
        assert!(
            validate_target_url("http://127.0.0.1:8080/image.png")
                .await
                .is_err()
        );
        assert!(
            validate_target_url("http://192.168.1.1/secret.jpg")
                .await
                .is_err()
        );
        assert!(
            validate_target_url("http://10.0.0.1/admin.png")
                .await
                .is_err()
        );
        assert!(
            validate_target_url("ftp://example.com/pic.png")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn test_validate_target_url_permits_public_url() {
        let res = validate_target_url(
            "https://pbs.twimg.com/profile_banners/1366430884698546176/1684330404/1500x500",
        )
        .await;
        assert!(res.is_ok());
    }

    #[test]
    fn test_url_cache_key_deterministic() {
        let key1 = url_cache_key("https://example.com/pic.jpg");
        let key2 = url_cache_key("https://example.com/pic.jpg");
        assert_eq!(key1, key2);
        assert_eq!(key1.len(), 64);
    }

    #[test]
    fn test_cache_write_and_read_roundtrip() {
        let dir = std::env::temp_dir().join(format!("mint-cache-test-{}", uuid::Uuid::new_v4()));
        let _ = fs::create_dir_all(&dir);
        let key = url_cache_key("https://example.com/sample.png");
        let payload = b"\x89PNG fake image content";

        let write_res = write_to_cache(
            &dir,
            &key,
            "https://example.com/sample.png",
            "image/png",
            payload,
        );
        assert!(write_res.is_ok());

        let cached = read_from_cache(&dir, &key);
        assert!(cached.is_some());
        let (content_type, bytes) = cached.unwrap();
        assert_eq!(content_type, "image/png");
        assert_eq!(bytes, payload);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_cache_expiration_prunes_stale_entry() {
        let dir = std::env::temp_dir().join(format!("mint-cache-test-{}", uuid::Uuid::new_v4()));
        let _ = fs::create_dir_all(&dir);
        let key = url_cache_key("https://example.com/expired.png");
        let bin_path = dir.join(format!("{key}.bin"));
        let meta_path = dir.join(format!("{key}.meta.json"));

        fs::write(&bin_path, b"expired data").unwrap();
        let old_time = 1000; // far in the past
        let meta = CacheMeta {
            url: "https://example.com/expired.png".into(),
            content_type: "image/png".into(),
            created_at: old_time,
            size: 12,
        };
        fs::write(&meta_path, serde_json::to_string(&meta).unwrap()).unwrap();

        let cached = read_from_cache(&dir, &key);
        assert!(cached.is_none());
        assert!(!bin_path.exists());
        assert!(!meta_path.exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_validate_target_url_rejects_non_http_schemes() {
        assert!(matches!(
            validate_target_url("file:///etc/passwd").await,
            Err(ImageProxyError::InvalidUrl(_))
        ));
        assert!(matches!(
            validate_target_url("ftp://example.com/pic.png").await,
            Err(ImageProxyError::InvalidUrl(_))
        ));
        assert!(matches!(
            validate_target_url("javascript:alert(1)").await,
            Err(ImageProxyError::InvalidUrl(_))
        ));
    }

    #[tokio::test]
    async fn test_validate_target_url_blocks_private_ips() {
        assert!(matches!(
            validate_target_url("http://192.168.1.100/test.png").await,
            Err(ImageProxyError::SsrfBlocked(_))
        ));
        assert!(matches!(
            validate_target_url("http://10.10.10.10/test.png").await,
            Err(ImageProxyError::SsrfBlocked(_))
        ));
        assert!(matches!(
            validate_target_url("http://172.20.0.1/test.png").await,
            Err(ImageProxyError::SsrfBlocked(_))
        ));
        assert!(matches!(
            validate_target_url("http://[::1]/test.png").await,
            Err(ImageProxyError::SsrfBlocked(_))
        ));
    }

    #[test]
    fn test_prune_cache_lru() {
        let dir =
            std::env::temp_dir().join(format!("mint-cache-test-prune-{}", uuid::Uuid::new_v4()));
        let _ = fs::create_dir_all(&dir);

        // Create 3 entries with varying creation times
        let entries = [
            ("key1", 100, vec![b'a'; 1000]),
            ("key2", 200, vec![b'b'; 1000]),
            ("key3", 300, vec![b'c'; 1000]),
        ];

        for (k, ts, data) in &entries {
            let bin = dir.join(format!("{k}.bin"));
            let meta = dir.join(format!("{k}.meta.json"));
            fs::write(&bin, data).unwrap();
            let c_meta = CacheMeta {
                url: format!("https://example.com/{k}"),
                content_type: "image/jpeg".into(),
                created_at: *ts,
                size: data.len(),
            };
            fs::write(&meta, serde_json::to_string(&c_meta).unwrap()).unwrap();
        }

        // Total data is ~3000 bytes. If we prune when size exceeds 2500 down to 1500,
        // key1 (oldest, ts=100) and then key2 (ts=200) should be pruned.
        let pruned = prune_cache_to_size(&dir, 2500, 1500).unwrap();
        assert!(pruned >= 1000);
        assert!(!dir.join("key1.bin").exists());
        assert!(dir.join("key3.bin").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_default_cache_dir_path_structure() {
        let dir = default_cache_dir();
        let s = dir.to_string_lossy();
        assert!(
            s.ends_with("cache/image-proxy") || s.ends_with("mint-image-proxy"),
            "default_cache_dir must point to cache/image-proxy, got: {s}"
        );
    }

    #[test]
    fn test_cleanup_legacy_cache_dir_removes_old_directory() {
        let temp_root =
            std::env::temp_dir().join(format!("mint-legacy-test-{}", uuid::Uuid::new_v4()));
        let legacy_dir = temp_root.join("Pictures").join("image-cache");
        fs::create_dir_all(&legacy_dir).unwrap();
        let dummy_file = legacy_dir.join("orphaned.bin");
        fs::write(&dummy_file, b"old-data").unwrap();
        assert!(dummy_file.is_file());

        // Test the real cleanup function with path parameter injection
        let cleaned = cleanup_legacy_cache_dir_in(Some(&legacy_dir));
        assert!(cleaned);
        assert!(!legacy_dir.exists());

        // Non-existent path returns false
        assert!(!cleanup_legacy_cache_dir_in(Some(&legacy_dir)));
        assert!(!cleanup_legacy_cache_dir_in(None));

        let _ = fs::remove_dir_all(&temp_root);
    }
}

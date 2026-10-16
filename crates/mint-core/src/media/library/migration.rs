use super::*;

impl LibraryStore {
    pub(super) fn migrate(&self) -> Result<()> {
        let mut connection = self.memory.connection()?;
        if connection
            .query_row(
                "SELECT 1 FROM library_migrations WHERE name='pictures-v1'",
                [],
                |_| Ok(()),
            )
            .optional()?
            .is_some()
        {
            return Ok(());
        }
        let index_path = self.legacy.join("pictures.json");
        let entries: Vec<crate::PictureEntry> = if index_path.exists() {
            let raw = fs::read(&index_path)?;
            let entries = serde_json::from_slice(&raw)?;
            let backup = self.legacy.join("pictures.json.library-backup");
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(backup)
            {
                Ok(mut file) => {
                    use std::io::Write;
                    file.write_all(&raw)?;
                    file.sync_all()?;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
            entries
        } else {
            vec![]
        };
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for entry in entries {
            let kind = if entry.mime_type.starts_with("video/") {
                "video"
            } else {
                "image"
            };
            let thumbnail = entry.thumbnail_path.filter(|p| p.exists());
            tx.execute("INSERT OR IGNORE INTO library_items (id,name,filename,mime_type,kind,size,created_at,origin,source,message,path,thumbnail_path,thumbnail_status) VALUES (?1,?2,?2,?3,?4,?5,?6,'legacy',?7,?8,?9,?10,?11)", params![entry.id, entry.filename, entry.mime_type, kind, fs::metadata(&entry.path).map(|m|m.len()).unwrap_or(0), entry.created_at, entry.source, entry.message, entry.path.to_string_lossy(), thumbnail.as_ref().map(|p|p.to_string_lossy().into_owned()), if thumbnail.is_some() { "ready" } else { "pending" }])?;
        }
        tx.execute(
            "INSERT OR IGNORE INTO library_migrations(name) VALUES('pictures-v1')",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }
}

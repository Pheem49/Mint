use super::*;

impl LibraryStore {
    /// Forget a Library Trash entry only after its remaining files reach system Trash.
    pub fn remove(&self, id: &str) -> Result<()> {
        self.remove_with(id, |path| trash::delete(path).map_err(|e| e.to_string()))
    }

    pub(super) fn remove_with(
        &self,
        id: &str,
        mut move_file: impl FnMut(&Path) -> std::result::Result<(), String>,
    ) -> Result<()> {
        let mut connection = self.memory.connection()?;
        // Serialize removal against restore, duplicate removal, and thumbnail publication.
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let item = self.get(id)?;
        if item.deleted_at.is_none() {
            return Err(invalid(
                "Move this item to Library Trash before removing it from Mint.",
            ));
        }
        let mut candidates = vec![item.path.clone()];
        candidates.extend(item.thumbnail_path.clone());
        // A worker may have written its preview but not published its path yet.
        if Path::new(id).file_name().and_then(|name| name.to_str()) == Some(id) {
            candidates.push(self.root.join("thumbnails").join(format!("{id}.thumb.png")));
        }
        let mut paths = std::collections::BTreeSet::new();
        // Validate every existing file before moving any of them. Missing originals
        // are expected for stale Pictures records and partially completed retries.
        for path in candidates {
            match fs::symlink_metadata(&path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(invalid(
                        "Cannot remove a symbolic link from Library storage.",
                    ));
                }
                Ok(_) => {
                    paths.insert(self.managed_path(&path)?);
                }
            }
        }
        for path in paths {
            move_file(&path).map_err(|error| invalid(format!("Could not finish moving files to system Trash: {error}. The entry remains in Mint; retry to finish.")))?;
        }
        tx.execute("DELETE FROM conversation_source_attachments WHERE interaction_id IN (SELECT interaction_id FROM library_references WHERE item_id=?1) AND json_extract(metadata,'$.name')=?2 AND json_extract(metadata,'$.kind')=?3",params![id,item.name,item.kind])?;
        tx.execute("DELETE FROM library_references WHERE item_id=?1", [id])?;
        tx.execute("DELETE FROM library_items WHERE id=?1", [id])?;
        tx.commit()?;
        Ok(())
    }
}

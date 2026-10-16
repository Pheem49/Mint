use super::*;

fn inflight() -> &'static Mutex<std::collections::HashSet<(PathBuf, String)>> {
    static JOBS: OnceLock<Mutex<std::collections::HashSet<(PathBuf, String)>>> = OnceLock::new();
    JOBS.get_or_init(Default::default)
}
impl LibraryStore {
    pub(super) fn enqueue_thumbnail(&self, id: String) {
        let key = (self.root.clone(), id.clone());
        if !inflight().lock().unwrap().insert(key.clone()) {
            return;
        }
        type Job = (LibraryStore, String);
        static QUEUE: OnceLock<mpsc::Sender<Job>> = OnceLock::new();
        let sender = QUEUE.get_or_init(|| {
            let (tx, rx) = mpsc::channel::<Job>();
            let rx = Arc::new(Mutex::new(rx));
            for _ in 0..2 {
                let rx = rx.clone();
                std::thread::spawn(move || {
                    loop {
                        let job = { rx.lock().unwrap().recv() };
                        let Ok((store, id)) = job else { break };
                        let _ = store.make_thumbnail(&id);
                        inflight().lock().unwrap().remove(&(store.root.clone(), id));
                    }
                });
            }
            tx
        });
        if sender.send((self.clone(), id)).is_err() {
            inflight().lock().unwrap().remove(&key);
        }
    }
    fn make_thumbnail(&self, id: &str) -> Result<()> {
        let item = self.get(id)?;
        if item.thumbnail_status != "pending" {
            return Ok(());
        }
        let thumbnail = self
            .managed_path(&item.path)
            .ok()
            .and_then(|path| fs::read(path).ok())
            .and_then(|bytes| {
                if item.kind == "video" {
                    crate::pictures::extract_video_thumbnail_from_bytes(&self.root, id, &bytes)
                } else {
                    crate::pictures::create_thumbnail_from_bytes(&self.root, id, &bytes)
                }
            });
        let updated = self.memory.connection()?.execute(
            "UPDATE library_items SET thumbnail_path=?2,thumbnail_status=?3 WHERE id=?1",
            params![
                id,
                thumbnail.as_ref().map(|p| p.to_string_lossy().into_owned()),
                if thumbnail.is_some() {
                    "ready"
                } else {
                    "unavailable"
                }
            ],
        )?;
        // Removal can finish while image/video decoding is in flight. Never leave
        // a newly generated preview behind after its catalog entry has gone.
        if updated == 0
            && let Some(path) = thumbnail
        {
            let _ = fs::remove_file(path);
        }
        Ok(())
    }
}

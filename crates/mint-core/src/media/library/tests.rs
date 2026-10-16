use super::*;
use std::fs;
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("mint-library-{}", uuid::Uuid::new_v4())))
    }
    fn store(&self) -> LibraryStore {
        LibraryStore::open(
            MemoryStore::open(self.0.join("memory.sqlite")),
            self.0.join("Library"),
            self.0.join("Pictures"),
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn document(name: &str) -> LibraryUpload {
    LibraryUpload {
        name: name.into(),
        data_uri: "data:text/markdown;base64,aGVsbG8=".into(),
    }
}

#[test]
fn thumbnails_are_generated_asynchronously_and_share_the_indexed_original() {
    let f = Fixture::new();
    let store = f.store();
    let image = image::DynamicImage::new_rgb8(800, 600);
    let mut encoded = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut encoded, image::ImageFormat::Png)
        .unwrap();
    let file = LibraryUpload {
        name: "reference.png".into(),
        data_uri: format!(
            "data:image/png;base64,{}",
            STANDARD.encode(encoded.into_inner())
        ),
    };
    let item = store
        .import(&[file], "image", Some("A"), "uploaded", "chat", "")
        .unwrap()
        .remove(0);
    for _ in 0..100 {
        if store.get(&item.id).unwrap().thumbnail_status != "pending" {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let (_, bytes) = store.read(&item.id, true).unwrap();
    let thumbnail = image::load_from_memory(&bytes).unwrap();
    assert!(thumbnail.width() <= 360 && thumbnail.height() <= 360);
    let source = store
        .memory
        .conversation_sources("A")
        .unwrap()
        .attachments
        .remove(0);
    assert_eq!(source.library_item_id.as_deref(), Some(item.id.as_str()));
    assert_eq!(
        source.attachment.thumbnail_data_uri,
        Some(format!("data:image/png;base64,{}", STANDARD.encode(bytes)))
    );
    store.set_trashed(&item.id, true).unwrap();
    let source = store
        .memory
        .conversation_sources("A")
        .unwrap()
        .attachments
        .remove(0);
    assert!(source.trashed);
    assert!(source.attachment.thumbnail_data_uri.is_none());
    store.set_trashed(&item.id, false).unwrap();
    assert!(store.read(&item.id, true).is_ok());
}
#[test]
fn conflicting_retry_payload_is_rejected_and_indexed_reads_cannot_escape_storage() {
    let f = Fixture::new();
    let store = f.store();
    let item = store
        .import(
            &[document("one.md")],
            "same",
            Some("A"),
            "uploaded",
            "chat",
            "",
        )
        .unwrap()
        .remove(0);
    assert!(
        store
            .import(
                &[document("two.md")],
                "same",
                Some("A"),
                "uploaded",
                "chat",
                ""
            )
            .is_err()
    );
    assert!(
        store
            .import(
                &[document("one.md"), document("extra.md")],
                "same",
                Some("A"),
                "uploaded",
                "chat",
                ""
            )
            .is_err()
    );
    let outside = f.0.join("private.md");
    fs::write(&outside, b"private").unwrap();
    store
        .memory
        .connection()
        .unwrap()
        .execute(
            "UPDATE library_items SET path=?2 WHERE id=?1",
            params![item.id, outside.to_string_lossy()],
        )
        .unwrap();
    assert!(store.read(&item.id, false).is_err());
    assert_eq!(store.get(&item.id).unwrap().availability, "missing");
    assert!(
        serde_json::to_string(&store.get(&item.id).unwrap())
            .unwrap()
            .find("private.md")
            .is_none()
    );
}
#[test]
fn unbound_references_are_removed_without_deleting_originals() {
    for delete in [false, true] {
        let f = Fixture::new();
        let store = f.store();
        let item = store
            .import(
                &[document("draft.md")],
                "pending",
                Some("A"),
                "uploaded",
                "chat",
                "",
            )
            .unwrap()
            .remove(0);
        if delete {
            store.memory.delete_chat_session("A").unwrap();
        } else {
            store.memory.clear_interactions_for_chat("A").unwrap();
        }
        assert!(store.get(&item.id).unwrap().references.is_empty());
        assert_eq!(store.read(&item.id, false).unwrap().1, b"hello");
    }
}
#[test]
fn intake_saves_original_before_extraction_and_reuses_retry_receipt() {
    let f = Fixture::new();
    let store = f.store();
    let request = crate::ChatRequest {
        message: "Read".into(),
        chat_id: Some("A".into()),
        document_attachment: Some(crate::chat::DocumentAttachment {
            filename: "สรุป.md".into(),
            data_uri: document("x.md").data_uri,
        }),
        ..Default::default()
    };
    let metadata = AttachmentMetadata {
        request_id: "submission".into(),
        ..Default::default()
    };
    let (enriched, receipt) = store
        .prepare(&request, Some(&metadata), &crate::MintConfig::default())
        .unwrap();
    assert!(enriched.message.contains("hello"));
    assert!(enriched.document_attachment.is_none());
    let receipt = receipt.unwrap();
    let sources = store.memory.conversation_sources("A").unwrap();
    assert_eq!(sources.attachments.len(), 1);
    assert!(sources.attachments[0].library_item_id.is_some());
    assert!(sources.attachments[0].interaction_id.is_none());
    store
        .prepare(&request, Some(&metadata), &crate::MintConfig::default())
        .unwrap();
    assert_eq!(store.list(&LibraryQuery::default()).unwrap().items.len(), 1);
    let turn = store.memory.start_turn("A", "Read").unwrap();
    receipt.bind(turn).unwrap();
    assert_eq!(
        store.list(&LibraryQuery::default()).unwrap().items[0].references[0].interaction_id,
        Some(turn)
    );
    let bad = crate::ChatRequest {
        document_attachment: Some(crate::chat::DocumentAttachment {
            filename: "broken.pdf".into(),
            data_uri: "data:application/pdf;base64,aGVsbG8=".into(),
        }),
        ..request
    };
    let bad_meta = AttachmentMetadata {
        request_id: "failed-extraction".into(),
        ..Default::default()
    };
    assert!(
        store
            .prepare(&bad, Some(&bad_meta), &crate::MintConfig::default())
            .is_err()
    );
    assert_eq!(store.list(&LibraryQuery::default()).unwrap().items.len(), 2);
}
#[test]
fn original_bytes_names_and_receipts_survive_reopen_and_retry() {
    let f = Fixture::new();
    let store = f.store();
    let first = store
        .import(
            &[document("สรุป.md")],
            "request-1",
            Some("A"),
            "uploaded",
            "chat",
            "Read",
        )
        .unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].name, "สรุป.md");
    let again = f
        .store()
        .import(
            &[document("สรุป.md")],
            "request-1",
            Some("A"),
            "uploaded",
            "chat",
            "Read",
        )
        .unwrap();
    assert_eq!(again[0].id, first[0].id);
    assert_eq!(store.read(&first[0].id, false).unwrap().1, b"hello");
    let another = store
        .import(
            &[document("สรุป.md")],
            "request-2",
            Some("B"),
            "uploaded",
            "chat",
            "Read",
        )
        .unwrap();
    assert_ne!(another[0].id, first[0].id);
    assert_eq!(
        store
            .list(&LibraryQuery {
                chat_id: Some("A".into()),
                ..Default::default()
            })
            .unwrap()
            .items
            .len(),
        1
    );
    let turn = store
        .memory
        .add_interaction_for_chat("A", "Read", "reply", "test", "test")
        .unwrap();
    store.bind_request("A", "request-1", turn).unwrap();
    store.memory.clear_interactions_for_chat("A").unwrap();
    assert_eq!(store.read(&first[0].id, false).unwrap().1, b"hello");
    assert!(store.get(&first[0].id).unwrap().references.is_empty());
    store.set_trashed(&first[0].id, true).unwrap();
    assert!(store.read(&first[0].id, false).is_err());
    assert_eq!(
        store
            .list(&LibraryQuery {
                trashed: true,
                ..Default::default()
            })
            .unwrap()
            .items
            .len(),
        1
    );
    store.set_trashed(&first[0].id, false).unwrap();
    assert_eq!(store.read(&first[0].id, false).unwrap().1, b"hello");
}

#[test]
fn migration_is_repeatable_keeps_ids_and_lists_missing_originals() {
    let f = Fixture::new();
    let old = f.0.join("Pictures");
    fs::create_dir_all(&old).unwrap();
    let path = old.join("old.png");
    fs::write(&path, b"old image").unwrap();
    let index = serde_json::json!([{"id":"old-id","filename":"old.png","path":path,"mimeType":"image/png","createdAt":"2026-01-01T00:00:00Z","source":"chat","message":"old"}, {"id":"missing","filename":"missing.png","path":old.join("missing.png"),"mimeType":"image/png","createdAt":"2026-01-01T00:00:00Z","source":"chat","message":"gone"}]);
    fs::write(old.join("pictures.json"), index.to_string()).unwrap();
    let store = f.store();
    assert_eq!(store.list(&LibraryQuery::default()).unwrap().items.len(), 2);
    assert_eq!(store.read("old-id", false).unwrap().1, b"old image");
    assert_eq!(store.get("missing").unwrap().availability, "missing");
    assert!(old.join("pictures.json.library-backup").exists());
    assert_eq!(
        f.store()
            .list(&LibraryQuery::default())
            .unwrap()
            .items
            .len(),
        2
    );
    assert_eq!(fs::read(path).unwrap(), b"old image");
}

#[test]
fn concurrent_imports_and_cursor_pages_do_not_lose_items() {
    let f = Fixture::new();
    let store = f.store();
    std::thread::scope(|scope| {
        for i in 0..45 {
            let store = &store;
            scope.spawn(move || {
                store
                    .import(
                        &[document(&format!("file-{i}.md"))],
                        &format!("request-{i}"),
                        None,
                        "uploaded",
                        "chat",
                        "",
                    )
                    .unwrap();
            });
        }
    });
    let first = store.list(&LibraryQuery::default()).unwrap();
    assert_eq!(first.items.len(), 40);
    let second = store
        .list(&LibraryQuery {
            cursor: first.next_cursor,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(second.items.len(), 5);
    assert!(second.next_cursor.is_none());
    let unique: std::collections::HashSet<_> = first
        .items
        .iter()
        .chain(&second.items)
        .map(|item| &item.id)
        .collect();
    assert_eq!(unique.len(), 45);
    assert_eq!(
        store
            .list(&LibraryQuery {
                search: Some("file-1.md".into()),
                ..Default::default()
            })
            .unwrap()
            .items
            .len(),
        1
    );
}

#[test]
fn failed_catalog_batch_and_concurrent_retries_keep_only_committed_originals() {
    let f = Fixture::new();
    let store = f.store();
    store.memory.connection().unwrap().execute_batch("CREATE TRIGGER reject_batch BEFORE INSERT ON library_references WHEN NEW.request_id='blocked' BEGIN SELECT RAISE(ABORT,'blocked'); END;").unwrap();
    assert!(
        store
            .import(
                &[document("one.md"), document("two.md")],
                "blocked",
                Some("A"),
                "uploaded",
                "chat",
                ""
            )
            .is_err()
    );
    assert_eq!(fs::read_dir(store.root.join("files")).unwrap().count(), 0);
    assert!(
        store
            .list(&LibraryQuery::default())
            .unwrap()
            .items
            .is_empty()
    );
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let store = &store;
                scope.spawn(move || {
                    store
                        .import(
                            &[document("one.md")],
                            "retry",
                            Some("A"),
                            "uploaded",
                            "chat",
                            "",
                        )
                        .unwrap()
                        .remove(0)
                        .id
                })
            })
            .collect();
        let ids: std::collections::HashSet<_> = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect();
        assert_eq!(ids.len(), 1);
    });
    assert_eq!(fs::read_dir(store.root.join("files")).unwrap().count(), 1);
    assert!(
        store
            .import(
                &[document("changed.md")],
                "retry",
                Some("A"),
                "uploaded",
                "chat",
                ""
            )
            .is_err()
    );
    assert_eq!(fs::read_dir(store.root.join("files")).unwrap().count(), 1);
}

#[test]
fn invalid_batches_and_file_write_failures_leave_no_catalog_entries() {
    let f = Fixture::new();
    let store = f.store();
    let bad = LibraryUpload {
        name: "archive.zip".into(),
        data_uri: "data:application/zip;base64,aGVsbG8=".into(),
    };
    assert!(
        store
            .import(
                &[document("good.md"), bad],
                "bad",
                None,
                "uploaded",
                "chat",
                ""
            )
            .is_err()
    );
    fs::create_dir_all(&store.root).unwrap();
    fs::write(store.root.join("files"), b"blocked").unwrap();
    assert!(
        store
            .import(
                &[document("good.md")],
                "blocked",
                None,
                "uploaded",
                "chat",
                ""
            )
            .is_err()
    );
    assert!(
        store
            .list(&LibraryQuery::default())
            .unwrap()
            .items
            .is_empty()
    );
}

#[test]
fn removal_requires_library_trash_and_moves_original_and_thumbnail_before_forgetting_refs() {
    let f = Fixture::new();
    let store = f.store();
    let item = store
        .import(
            &[document("notes.md")],
            "remove",
            Some("A"),
            "uploaded",
            "chat",
            "",
        )
        .unwrap()
        .remove(0);
    let turn = store.memory.start_turn("A", "Read notes").unwrap();
    store.bind_request("A", "remove", turn).unwrap();
    store
        .memory
        .save_source_attachments(
            "A",
            turn,
            &[crate::agent::memory::SourceAttachment {
                name: "notes.md".into(),
                kind: "file".into(),
                mime_type: "text/markdown".into(),
                thumbnail_data_uri: None,
            }],
        )
        .unwrap();
    let thumb = store
        .root
        .join("thumbnails")
        .join(format!("{}.thumb.png", item.id));
    fs::create_dir_all(thumb.parent().unwrap()).unwrap();
    fs::write(&thumb, b"preview").unwrap();
    store
        .memory
        .connection()
        .unwrap()
        .execute(
            "UPDATE library_items SET thumbnail_path=?2,thumbnail_status='ready' WHERE id=?1",
            params![item.id, thumb.to_string_lossy()],
        )
        .unwrap();
    assert!(
        store
            .remove_with(&item.id, |_| panic!("active files cannot be removed"))
            .is_err()
    );
    store.set_trashed(&item.id, true).unwrap();
    let trash = f.0.join("system-trash");
    fs::create_dir(&trash).unwrap();
    store
        .remove_with(&item.id, |path| {
            fs::rename(path, trash.join(path.file_name().unwrap())).map_err(|e| e.to_string())
        })
        .unwrap();
    assert_eq!(
        fs::read(trash.join(item.path.file_name().unwrap())).unwrap(),
        b"hello"
    );
    assert_eq!(
        fs::read(trash.join(thumb.file_name().unwrap())).unwrap(),
        b"preview"
    );
    assert!(store.get(&item.id).is_err());
    assert!(
        store
            .memory
            .conversation_sources("A")
            .unwrap()
            .attachments
            .is_empty()
    );
    assert_eq!(
        store
            .memory
            .recent_interactions_for_chat("A", 10)
            .unwrap()
            .len(),
        1
    );
}
#[test]
fn removal_forgets_missing_originals_and_keeps_catalog_when_system_trash_fails() {
    let f = Fixture::new();
    let store = f.store();
    let item = store
        .import(
            &[document("missing.md")],
            "missing",
            None,
            "uploaded",
            "chat",
            "",
        )
        .unwrap()
        .remove(0);
    store.set_trashed(&item.id, true).unwrap();
    assert!(
        store
            .remove_with(&item.id, |_| Err("Trash unavailable".into()))
            .is_err()
    );
    assert!(store.get(&item.id).is_ok());
    assert_eq!(fs::read(&item.path).unwrap(), b"hello");
    fs::remove_file(&item.path).unwrap();
    store
        .remove_with(&item.id, |_| {
            panic!("missing files must not be sent to Trash")
        })
        .unwrap();
    assert!(store.get(&item.id).is_err());
}
#[test]
fn removal_rejects_unmanaged_files_before_moving_anything() {
    let f = Fixture::new();
    let store = f.store();
    let item = store
        .import(
            &[document("safe.md")],
            "unsafe",
            None,
            "uploaded",
            "chat",
            "",
        )
        .unwrap()
        .remove(0);
    store.set_trashed(&item.id, true).unwrap();
    let outside = f.0.join("private.png");
    fs::write(&outside, b"private").unwrap();
    store
        .memory
        .connection()
        .unwrap()
        .execute(
            "UPDATE library_items SET thumbnail_path=?2 WHERE id=?1",
            params![item.id, outside.to_string_lossy()],
        )
        .unwrap();
    assert!(
        store
            .remove_with(&item.id, |_| panic!("paths must all be validated first"))
            .is_err()
    );
    assert!(item.path.exists());
    assert!(outside.exists());
    assert!(store.get(&item.id).is_ok());
}

#[test]
fn removal_can_retry_after_only_some_files_reached_system_trash() {
    let f = Fixture::new();
    let store = f.store();
    let item = store
        .import(
            &[document("partial.md")],
            "partial",
            None,
            "uploaded",
            "chat",
            "",
        )
        .unwrap()
        .remove(0);
    store.set_trashed(&item.id, true).unwrap();
    let thumb = store
        .root
        .join("thumbnails")
        .join(format!("{}.thumb.png", item.id));
    fs::create_dir_all(thumb.parent().unwrap()).unwrap();
    fs::write(&thumb, b"preview").unwrap();
    store
        .memory
        .connection()
        .unwrap()
        .execute(
            "UPDATE library_items SET thumbnail_path=?2 WHERE id=?1",
            params![item.id, thumb.to_string_lossy()],
        )
        .unwrap();
    let trash = f.0.join("system-trash");
    fs::create_dir(&trash).unwrap();
    assert!(
        store
            .remove_with(&item.id, |path| if path == thumb {
                Err("Trash unavailable".into())
            } else {
                fs::rename(path, trash.join(path.file_name().unwrap())).map_err(|e| e.to_string())
            })
            .is_err()
    );
    assert!(!item.path.exists());
    assert!(thumb.exists());
    assert!(store.get(&item.id).is_ok());
    store
        .remove_with(&item.id, |path| {
            fs::rename(path, trash.join(path.file_name().unwrap())).map_err(|e| e.to_string())
        })
        .unwrap();
    assert!(store.get(&item.id).is_err());
    assert_eq!(fs::read(trash.join(item.filename)).unwrap(), b"hello");
    assert_eq!(
        fs::read(trash.join(thumb.file_name().unwrap())).unwrap(),
        b"preview"
    );
}

#[test]
fn generated_media_keeps_its_running_conversation_and_not_a_newer_queued_turn() {
    let f = Fixture::new();
    let store = f.store();
    let turn = store.memory.start_turn("A", "Generate").unwrap();
    assert!(store.memory.claim_turn("A", turn).unwrap());
    let _queued = store.memory.start_turn("A", "Next").unwrap();
    let item = store
        .import(
            &[document("output.md")],
            "generated",
            None,
            "generated",
            "test",
            "prompt",
        )
        .unwrap()
        .remove(0);
    store
        .link_generated(std::slice::from_ref(&item.id), "A")
        .unwrap();
    assert_eq!(
        store
            .list(&LibraryQuery {
                chat_id: Some("A".into()),
                ..Default::default()
            })
            .unwrap()
            .items
            .len(),
        1
    );
    let refs = store.get(&item.id).unwrap().references;
    assert_eq!(refs[0].interaction_id, Some(turn));
    store
        .link_generated(std::slice::from_ref(&item.id), "A")
        .unwrap();
    assert_eq!(store.get(&item.id).unwrap().references.len(), 1);
}

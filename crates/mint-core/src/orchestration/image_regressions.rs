use super::*;

#[tokio::test]
async fn parallel_image_reads_preserve_pixels_for_each_model_tool_call() {
    let root = std::env::temp_dir().join(format!("mint-image-batch-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    image::RgbImage::new(4, 3)
        .save(root.join("source.png"))
        .unwrap();
    let mut config = MintConfig::default();
    config.allowed_read_paths.push(root.clone());
    let decisions: Vec<_> = (0..2)
        .map(|index| {
            (
                format!("image-{index}"),
                AgentDecision {
                    thought: String::new(),
                    action: "view_image".into(),
                    input: AgentInput {
                        path: "source.png".into(),
                        ..Default::default()
                    },
                },
            )
        })
        .collect();
    let mut images = std::collections::HashMap::new();
    let mut artifacts = Vec::new();
    let results = run_parallel_read_only_batch(
        &decisions,
        1,
        &root,
        &config,
        "batch-images",
        &mut |_| Ok(ApprovalOutcome::Approved),
        &mut |event| {
            if let AgentProgress::ToolArtifacts {
                artifacts: entries, ..
            } = event
            {
                artifacts.extend(entries);
            }
        },
        &mut BTreeMap::new(),
        &mut Vec::new(),
        &mut images,
        &[],
        None,
        true,
    )
    .await;
    let directory = crate::mcp_result::artifact_directory().unwrap();
    for entry in artifacts {
        let _ = std::fs::remove_file(directory.join(&entry.id));
        let _ = std::fs::remove_file(directory.join(format!("{}.json", entry.id)));
    }
    let _ = std::fs::remove_dir_all(root);
    assert_eq!(results.len(), 2);
    assert_eq!(images.get("image-0").map(Vec::len), Some(1));
    assert_eq!(images.get("image-1").map(Vec::len), Some(1));
}

#[test]
fn deleting_a_conversation_removes_only_its_image_artifacts() {
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::RgbImage::new(2, 2)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    let owner = format!("image-delete-{}", uuid::Uuid::new_v4());
    let image = crate::media::tool_images::save(
        bytes.get_ref(),
        "image/png",
        "result.png",
        &owner,
        "call",
        "browser",
        None,
    )
    .unwrap();
    let other = crate::media::tool_images::save(
        bytes.get_ref(),
        "image/png",
        "other.png",
        &format!("{owner}-other"),
        "call",
        "browser",
        None,
    )
    .unwrap();
    let artifact = &image.artifacts[0];
    let database =
        std::env::temp_dir().join(format!("mint-image-delete-{}.sqlite", uuid::Uuid::new_v4()));
    let memory = MemoryStore::open(&database);
    memory.delete_chat_session(&owner).unwrap();
    let present = crate::mcp_result::read_artifact(&artifact.id).is_ok();
    let other_present = crate::mcp_result::read_artifact(&other.artifacts[0].id).is_ok();
    let directory = crate::mcp_result::artifact_directory().unwrap();
    let _ = std::fs::remove_file(directory.join(&artifact.id));
    let _ = std::fs::remove_file(directory.join(format!("{}.json", artifact.id)));
    let _ = std::fs::remove_file(directory.join(&other.artifacts[0].id));
    let _ = std::fs::remove_file(directory.join(format!("{}.json", other.artifacts[0].id)));
    let _ = std::fs::remove_file(database);
    assert!(!present, "deleted conversation left its screenshot on disk");
    assert!(
        other_present,
        "deleting one chat removed another chat's image"
    );
}

#[tokio::test]
async fn viewed_images_have_durable_artifacts_and_separate_model_pixels() {
    let root = std::env::temp_dir().join(format!("mint-image-outcome-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    image::RgbImage::new(4, 3)
        .save(root.join("source.png"))
        .unwrap();
    let mut config = MintConfig::default();
    config.allowed_read_paths.push(root.clone());
    let decision = AgentDecision {
        thought: String::new(),
        action: "view_image".into(),
        input: AgentInput {
            path: "source.png".into(),
            ..Default::default()
        },
    };
    let outcome = execute_tool_outcome_reviewed(
        &root,
        &config,
        &decision,
        "image-owner",
        &mut |_| Ok(ApprovalOutcome::Approved),
        &mut |_| {},
        "image-call",
        None,
    )
    .await
    .unwrap();
    let _ = std::fs::remove_dir_all(root);
    assert_eq!(outcome.images.len(), 1);
    assert_eq!(outcome.artifacts.len(), 1);
    assert!(!outcome.text.contains("base64"));
    let artifact = &outcome.artifacts[0];
    let (_, bytes) = crate::mcp_result::read_artifact(&artifact.id).unwrap();
    assert!(image::load_from_memory(&bytes).is_ok());
    assert!(crate::media::tool_images::view_artifact(&artifact.id, "different-chat").is_err());
    assert_eq!(
        crate::media::tool_images::view_artifact(&artifact.id, "image-owner")
            .unwrap()
            .images
            .len(),
        1
    );
    let directory = crate::mcp_result::artifact_directory().unwrap();
    let _ = std::fs::remove_file(directory.join(&artifact.id));
    let _ = std::fs::remove_file(directory.join(format!("{}.json", artifact.id)));
}

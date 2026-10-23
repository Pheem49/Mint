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

#[tokio::test]
async fn orchestration_prunes_older_tool_images_while_preserving_recent_images() {
    let mut messages = Vec::new();
    let mut images_1 = std::collections::HashMap::new();
    images_1.insert("call-1".into(), vec!["data:image/png;base64,step1".into()]);
    let results_1 = vec![ToolResultEntry {
        call_id: "call-1".into(),
        action: "view_image".into(),
        input: serde_json::json!({ "path": "step1.png" }),
        text: "[Image loaded — see attached image]".into(),
        status: crate::mcp_result::ToolStatus::Success,
    }];
    append_native_tool_results(&mut messages, "view step 1", &results_1, &Default::default(), &images_1);
    prune_historic_images_from_native_messages(&mut messages, MAX_RETAINED_IMAGE_STEPS);

    // Step 1: Image is retained (1 step <= MAX_RETAINED_IMAGE_STEPS)
    assert!(messages[1].content.iter().any(|b| matches!(b, ContentBlock::Image { .. })));

    let mut images_2 = std::collections::HashMap::new();
    images_2.insert("call-2".into(), vec!["data:image/png;base64,step2".into()]);
    let results_2 = vec![ToolResultEntry {
        call_id: "call-2".into(),
        action: "view_image".into(),
        input: serde_json::json!({ "path": "step2.png" }),
        text: "[Image loaded — see attached image]".into(),
        status: crate::mcp_result::ToolStatus::Success,
    }];
    append_native_tool_results(&mut messages, "view step 2", &results_2, &Default::default(), &images_2);
    prune_historic_images_from_native_messages(&mut messages, MAX_RETAINED_IMAGE_STEPS);

    // Step 2: Both images retained (2 steps <= MAX_RETAINED_IMAGE_STEPS)
    assert!(messages[1].content.iter().any(|b| matches!(b, ContentBlock::Image { .. })));
    assert!(messages[3].content.iter().any(|b| matches!(b, ContentBlock::Image { .. })));

    let mut images_3 = std::collections::HashMap::new();
    images_3.insert("call-3".into(), vec!["data:image/png;base64,step3".into()]);
    let results_3 = vec![ToolResultEntry {
        call_id: "call-3".into(),
        action: "view_image".into(),
        input: serde_json::json!({ "path": "step3.png" }),
        text: "[Image loaded — see attached image]".into(),
        status: crate::mcp_result::ToolStatus::Success,
    }];
    append_native_tool_results(&mut messages, "view step 3", &results_3, &Default::default(), &images_3);
    let pruned = prune_historic_images_from_native_messages(&mut messages, MAX_RETAINED_IMAGE_STEPS);

    // Step 3: Step 1's image is pruned! Steps 2 and 3 are retained.
    assert_eq!(pruned, 1);
    assert!(!messages[1].content.iter().any(|b| matches!(b, ContentBlock::Image { .. })));
    assert!(messages[3].content.iter().any(|b| matches!(b, ContentBlock::Image { .. })));
    assert!(messages[5].content.iter().any(|b| matches!(b, ContentBlock::Image { .. })));
    assert!(messages[1].content.iter().any(|b| matches!(b, ContentBlock::ToolResult { content, .. } if content.contains("historical image pixels pruned from context"))));
}


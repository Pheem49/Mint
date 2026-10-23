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

#[tokio::test]
async fn view_image_and_read_file_can_access_temp_directory_safely() {
    let workspace = std::env::temp_dir().join(format!("mint-workspace-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&workspace).unwrap();

    // 1. Create an image and a text file in std::env::temp_dir() outside the workspace
    let temp_img = std::env::temp_dir().join(format!("mint-temp-test-{}.png", uuid::Uuid::new_v4()));
    image::RgbImage::new(10, 10).save(&temp_img).unwrap();

    let temp_txt = std::env::temp_dir().join(format!("mint-temp-note-{}.txt", uuid::Uuid::new_v4()));
    std::fs::write(&temp_txt, "Hello from /tmp!").unwrap();

    let config = MintConfig::default();
    let mut approve = |_appr: &AgentApproval| Ok(ApprovalOutcome::Approved);

    // 2. Test view_image with absolute temp path
    let img_input = AgentInput {
        path: temp_img.to_string_lossy().to_string(),
        ..Default::default()
    };
    let img_res = tools::project::execute("view_image", &img_input, &workspace, &config, "test-chat", &mut approve).await;
    assert!(img_res.is_ok(), "view_image failed on temp file: {:?}", img_res.err());
    let img_out = img_res.unwrap();
    assert!(img_out.starts_with("data:image/png;base64,"));

    // 3. Test read_file with absolute temp path
    let txt_input = AgentInput {
        path: temp_txt.to_string_lossy().to_string(),
        ..Default::default()
    };
    let txt_res = tools::files::execute("read_file", &txt_input, &workspace, &config, "test-chat", &mut approve).await;
    assert!(txt_res.is_ok(), "read_file failed on temp file: {:?}", txt_res.err());
    let txt_out = txt_res.unwrap();
    assert!(txt_out.contains("Hello from /tmp!"));

    // 4. Test security: sensitive files like .env remain blocked even in temp
    let temp_sub = std::env::temp_dir().join(format!("mint-sub-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_sub).unwrap();
    let blocked_file = temp_sub.join(".env");
    std::fs::write(&blocked_file, "SECRET=123").unwrap();

    let blocked_input = AgentInput {
        path: blocked_file.to_string_lossy().to_string(),
        ..Default::default()
    };
    let blocked_res = tools::files::execute("read_file", &blocked_input, &workspace, &config, "test-chat", &mut approve).await;
    assert!(blocked_res.is_err(), "reading .env from temp should be blocked");

    // 5. Test orchestration::execute_tool interception path for view_image with /tmp (regression for mod.rs:3713)
    let decision = AgentDecision {
        thought: "view temp image".into(),
        action: "view_image".into(),
        input: AgentInput {
            path: temp_img.to_string_lossy().to_string(),
            ..Default::default()
        },
    };
    let mut progress_cb = |_p: AgentProgress| {};
    let outcome = execute_tool(
        &workspace,
        &config,
        &decision,
        "test-chat",
        &mut approve,
        &mut progress_cb,
    )
    .await;
    assert!(outcome.is_ok(), "orchestration execute_tool failed on temp image: {:?}", outcome.err());
    let outcome = outcome.unwrap();
    assert!(outcome.contains("Image saved:"));

    // Clean up
    let _ = std::fs::remove_file(temp_img);
    let _ = std::fs::remove_file(temp_txt);
    let _ = std::fs::remove_dir_all(temp_sub);
    let _ = std::fs::remove_dir_all(workspace);
}


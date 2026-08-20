use super::RequestCtx;
use tokio::net::TcpStream;

use super::super::*;

pub(in crate::api_server) async fn execute(ctx: RequestCtx<'_>, socket: TcpStream) {
    let RequestCtx {
        method,
        route,
        query: _query,
        body,
        request_str: _request_str,
        request_bytes: _request_bytes,
        header_end: _header_end,
        auth_label,
    } = ctx;
    match (method, route) {
        ("POST", "/api/image-generate") => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct ImageGenApiRequest {
                prompt: String,
                #[serde(default)]
                negative_prompt: Option<String>,
                #[serde(default)]
                aspect_ratio: Option<String>,
                #[serde(default)]
                num_images: Option<u8>,
                #[serde(default)]
                model: Option<String>,
                /// Which image provider to use (overrides config.image_gen_provider).
                #[serde(default)]
                provider: Option<String>,
                #[serde(default)]
                image_data_uri: Option<String>,
                #[serde(default)]
                mask_data_uri: Option<String>,
                #[serde(default)]
                mode: Option<String>,
            }

            if let Ok(req) = serde_json::from_str::<ImageGenApiRequest>(body) {
                let config = load_config().unwrap_or_default();
                let gen_request = ImageGenRequest {
                    prompt: req.prompt.clone(),
                    negative_prompt: req.negative_prompt,
                    aspect_ratio: req.aspect_ratio,
                    num_images: req.num_images,
                    model: req.model,
                    provider: req.provider,
                    image_data_uri: req.image_data_uri,
                    mask_data_uri: req.mask_data_uri,
                    mode: req.mode,
                };
                match generate_images(&config, &gen_request).await {
                    Ok(result) => {
                        log_api_req(
                            "POST",
                            "/api/image-generate",
                            "200 OK",
                            Some(&format!("Provider: {} | {}", result.provider, auth_label)),
                        );
                        let data_uris: Vec<String> = result
                            .images
                            .iter()
                            .map(|img| img.data_uri.clone())
                            .collect();
                        let mut saved = save_chat_images(
                            data_uris,
                            Some(result.provider.clone()),
                            Some(req.prompt.clone()),
                        )
                        .unwrap_or_default();
                        for picture in &mut saved {
                            picture.url = Some(format!("/api/pictures/{}", picture.filename));
                            picture.thumbnail_url =
                                Some(format!("/api/pictures/{}", picture.filename));
                        }
                        let response = json!({
                            "images": saved,
                            "model": result.model,
                            "provider": result.provider,
                            "prompt": result.prompt,
                            "description": result.description
                        });
                        send_json_response(socket, "200 OK", &response.to_string()).await;
                    }
                    Err(e) => {
                        log_api_err("API /api/image-generate error", &e);
                        let err = json!({ "error": e.to_string() });
                        send_json_response(socket, "500 Internal Server Error", &err.to_string())
                            .await;
                    }
                }
            } else {
                send_json_response(
                    socket,
                    "400 Bad Request",
                    "{\"error\":\"invalid image generation request body\"}",
                )
                .await;
            }
        }

        ("POST", "/api/video-generate") => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct VideoGenApiRequest {
                prompt: String,
                #[serde(default)]
                negative_prompt: Option<String>,
                #[serde(default)]
                aspect_ratio: Option<String>,
                #[serde(default)]
                duration: Option<u32>,
                #[serde(default)]
                model: Option<String>,
                #[serde(default)]
                provider: Option<String>,
            }

            if let Ok(req) = serde_json::from_str::<VideoGenApiRequest>(body) {
                let config = load_config().unwrap_or_default();
                let gen_request = VideoGenRequest {
                    prompt: req.prompt.clone(),
                    negative_prompt: req.negative_prompt,
                    aspect_ratio: req.aspect_ratio.unwrap_or_else(|| "16:9".to_string()),
                    duration: req.duration.unwrap_or(5),
                    model: req.model,
                    provider: req.provider.unwrap_or_else(|| "veo".to_string()),
                };
                match generate_video(&config, &gen_request).await {
                    Ok(result) => {
                        let mut response = serde_json::to_value(&result).unwrap_or(json!({}));
                        if let Some(videos) =
                            response.get_mut("videos").and_then(|v| v.as_array_mut())
                        {
                            for picture in videos {
                                let filename = picture
                                    .get("filename")
                                    .and_then(|f| f.as_str())
                                    .map(|s| s.to_string());
                                if let Some(filename) = filename {
                                    picture.as_object_mut().unwrap().insert(
                                        "url".to_string(),
                                        json!(format!("/api/pictures/{}", filename)),
                                    );
                                }
                                let id = picture
                                    .get("id")
                                    .and_then(|i| i.as_str())
                                    .map(|s| s.to_string());
                                if let Some(id) = id {
                                    let has_thumb = picture.get("thumbnailPath").is_some()
                                        || picture.get("thumbnailUrl").is_some();
                                    if has_thumb {
                                        picture.as_object_mut().unwrap().insert(
                                            "thumbnailUrl".to_string(),
                                            json!(format!("/api/thumbnails/{}.thumb.png", id)),
                                        );
                                    }
                                }
                            }
                        }
                        log_api_req(
                            "POST",
                            "/api/video-generate",
                            "200 OK",
                            Some(&format!("Provider: {} | {}", result.provider, auth_label)),
                        );
                        send_json_response(socket, "200 OK", &response.to_string()).await;
                    }
                    Err(e) => {
                        log_api_err("API /api/video-generate error", &e);
                        let err = json!({ "error": e.to_string() });
                        send_json_response(socket, "500 Internal Server Error", &err.to_string())
                            .await;
                    }
                }
            } else {
                send_json_response(
                    socket,
                    "400 Bad Request",
                    "{\"error\":\"invalid video generation request body\"}",
                )
                .await;
            }
        }
        (_, route) if route.starts_with("/api/video/") && method == "POST" => {
            // ── Video Editing Routes ────────────────────────────────────────────
            match route {
                "/api/video/load" => {
                    #[derive(serde::Deserialize)]
                    struct VideoLoadReq {
                        path: String,
                    }
                    if let Ok(req) = serde_json::from_str::<VideoLoadReq>(body) {
                        match video_load(&req.path) {
                            Ok(info) => {
                                let res = serde_json::to_string(&info).unwrap_or_default();
                                log_api_req("POST", "/api/video/load", "200 OK", None);
                                send_json_response(socket, "200 OK", &res).await;
                            }
                            Err(e) => {
                                log_api_err("/api/video/load", &e);
                                let err = json!({ "error": e.to_string() });
                                send_json_response(
                                    socket,
                                    "500 Internal Server Error",
                                    &err.to_string(),
                                )
                                .await;
                            }
                        }
                    } else {
                        send_json_response(
                            socket,
                            "400 Bad Request",
                            "{\"error\":\"expected {\\\"path\\\":\\\"...\\\"}\" }",
                        )
                        .await;
                    }
                }
                "/api/video/trim" => {
                    if let Ok(req) = serde_json::from_str::<TrimRequest>(body) {
                        match video_trim(&req) {
                            Ok(r) => {
                                let res = serde_json::to_string(&r).unwrap_or_default();
                                log_api_req("POST", "/api/video/trim", "200 OK", None);
                                send_json_response(socket, "200 OK", &res).await;
                            }
                            Err(e) => {
                                log_api_err("/api/video/trim", &e);
                                let err = json!({ "error": e.to_string() });
                                send_json_response(
                                    socket,
                                    "500 Internal Server Error",
                                    &err.to_string(),
                                )
                                .await;
                            }
                        }
                    } else {
                        send_json_response(
                            socket,
                            "400 Bad Request",
                            "{\"error\":\"invalid trim request\"}",
                        )
                        .await;
                    }
                }
                "/api/video/crop" => {
                    if let Ok(req) = serde_json::from_str::<CropRequest>(body) {
                        match video_crop(&req) {
                            Ok(r) => {
                                let res = serde_json::to_string(&r).unwrap_or_default();
                                log_api_req("POST", "/api/video/crop", "200 OK", None);
                                send_json_response(socket, "200 OK", &res).await;
                            }
                            Err(e) => {
                                log_api_err("/api/video/crop", &e);
                                let err = json!({ "error": e.to_string() });
                                send_json_response(
                                    socket,
                                    "500 Internal Server Error",
                                    &err.to_string(),
                                )
                                .await;
                            }
                        }
                    } else {
                        send_json_response(
                            socket,
                            "400 Bad Request",
                            "{\"error\":\"invalid crop request\"}",
                        )
                        .await;
                    }
                }
                "/api/video/resize" => {
                    if let Ok(req) = serde_json::from_str::<ResizeRequest>(body) {
                        match video_resize(&req) {
                            Ok(r) => {
                                let res = serde_json::to_string(&r).unwrap_or_default();
                                log_api_req("POST", "/api/video/resize", "200 OK", None);
                                send_json_response(socket, "200 OK", &res).await;
                            }
                            Err(e) => {
                                log_api_err("/api/video/resize", &e);
                                let err = json!({ "error": e.to_string() });
                                send_json_response(
                                    socket,
                                    "500 Internal Server Error",
                                    &err.to_string(),
                                )
                                .await;
                            }
                        }
                    } else {
                        send_json_response(
                            socket,
                            "400 Bad Request",
                            "{\"error\":\"invalid resize request\"}",
                        )
                        .await;
                    }
                }
                "/api/video/merge" => {
                    if let Ok(req) = serde_json::from_str::<MergeRequest>(body) {
                        match video_merge(&req) {
                            Ok(r) => {
                                let res = serde_json::to_string(&r).unwrap_or_default();
                                log_api_req("POST", "/api/video/merge", "200 OK", None);
                                send_json_response(socket, "200 OK", &res).await;
                            }
                            Err(e) => {
                                log_api_err("/api/video/merge", &e);
                                let err = json!({ "error": e.to_string() });
                                send_json_response(
                                    socket,
                                    "500 Internal Server Error",
                                    &err.to_string(),
                                )
                                .await;
                            }
                        }
                    } else {
                        send_json_response(
                            socket,
                            "400 Bad Request",
                            "{\"error\":\"invalid merge request\"}",
                        )
                        .await;
                    }
                }
                "/api/video/extract-audio" => {
                    if let Ok(req) = serde_json::from_str::<ExtractAudioRequest>(body) {
                        match video_extract_audio(&req) {
                            Ok(r) => {
                                let res = serde_json::to_string(&r).unwrap_or_default();
                                log_api_req("POST", "/api/video/extract-audio", "200 OK", None);
                                send_json_response(socket, "200 OK", &res).await;
                            }
                            Err(e) => {
                                log_api_err("/api/video/extract-audio", &e);
                                let err = json!({ "error": e.to_string() });
                                send_json_response(
                                    socket,
                                    "500 Internal Server Error",
                                    &err.to_string(),
                                )
                                .await;
                            }
                        }
                    } else {
                        send_json_response(
                            socket,
                            "400 Bad Request",
                            "{\"error\":\"invalid extract-audio request\"}",
                        )
                        .await;
                    }
                }
                "/api/video/remove-silence" => {
                    if let Ok(req) = serde_json::from_str::<RemoveSilenceRequest>(body) {
                        match video_remove_silence(&req) {
                            Ok(r) => {
                                let res = serde_json::to_string(&r).unwrap_or_default();
                                log_api_req("POST", "/api/video/remove-silence", "200 OK", None);
                                send_json_response(socket, "200 OK", &res).await;
                            }
                            Err(e) => {
                                log_api_err("/api/video/remove-silence", &e);
                                let err = json!({ "error": e.to_string() });
                                send_json_response(
                                    socket,
                                    "500 Internal Server Error",
                                    &err.to_string(),
                                )
                                .await;
                            }
                        }
                    } else {
                        send_json_response(
                            socket,
                            "400 Bad Request",
                            "{\"error\":\"invalid remove-silence request\"}",
                        )
                        .await;
                    }
                }
                "/api/video/export" => {
                    if let Ok(req) = serde_json::from_str::<ExportRequest>(body) {
                        match video_export(&req) {
                            Ok(r) => {
                                let res = serde_json::to_string(&r).unwrap_or_default();
                                log_api_req("POST", "/api/video/export", "200 OK", None);
                                send_json_response(socket, "200 OK", &res).await;
                            }
                            Err(e) => {
                                log_api_err("/api/video/export", &e);
                                let err = json!({ "error": e.to_string() });
                                send_json_response(
                                    socket,
                                    "500 Internal Server Error",
                                    &err.to_string(),
                                )
                                .await;
                            }
                        }
                    } else {
                        send_json_response(
                            socket,
                            "400 Bad Request",
                            "{\"error\":\"invalid export request\"}",
                        )
                        .await;
                    }
                }
                "/api/video/render-timeline" => {
                    if let Ok(req) = serde_json::from_str::<RenderTimelineRequest>(body) {
                        match render_timeline(&req.timeline) {
                            Ok(r) => {
                                let res = serde_json::to_string(&r).unwrap_or_default();
                                log_api_req(
                                    "POST",
                                    "/api/video/render-timeline",
                                    "200 OK",
                                    Some(&format!("{} clips", r.clips_rendered)),
                                );
                                send_json_response(socket, "200 OK", &res).await;
                            }
                            Err(e) => {
                                log_api_err("/api/video/render-timeline", &e);
                                let err = json!({ "error": e.to_string() });
                                send_json_response(
                                    socket,
                                    "500 Internal Server Error",
                                    &err.to_string(),
                                )
                                .await;
                            }
                        }
                    } else {
                        send_json_response(
                            socket,
                            "400 Bad Request",
                            "{\"error\":\"invalid timeline request\"}",
                        )
                        .await;
                    }
                }
                "/api/video/make-shorts" => {
                    if let Ok(req) = serde_json::from_str::<MakeShortsRequest>(body) {
                        let config = load_config().unwrap_or_default();
                        match make_shorts(&config, &req).await {
                            Ok(r) => {
                                let res = serde_json::to_string(&r).unwrap_or_default();
                                log_api_req(
                                    "POST",
                                    "/api/video/make-shorts",
                                    "200 OK",
                                    Some(&format!("{} shorts clips", r.clips.len())),
                                );
                                send_json_response(socket, "200 OK", &res).await;
                            }
                            Err(e) => {
                                log_api_err("/api/video/make-shorts", &e);
                                let err = json!({ "error": e.to_string() });
                                send_json_response(
                                    socket,
                                    "500 Internal Server Error",
                                    &err.to_string(),
                                )
                                .await;
                            }
                        }
                    } else {
                        send_json_response(
                            socket,
                            "400 Bad Request",
                            "{\"error\":\"invalid make-shorts request body\"}",
                        )
                        .await;
                    }
                }
                "/api/video/ai-edit" => {
                    if let Ok(req) = serde_json::from_str::<AiEditVideoRequest>(body) {
                        let config = load_config().unwrap_or_default();
                        match ai_edit_video(&config, &req).await {
                            Ok(r) => {
                                let res = serde_json::to_string(&r).unwrap_or_default();
                                log_api_req(
                                    "POST",
                                    "/api/video/ai-edit",
                                    "200 OK",
                                    Some(&format!("AI executed prompt: {}", req.instruction)),
                                );
                                send_json_response(socket, "200 OK", &res).await;
                            }
                            Err(e) => {
                                log_api_err("/api/video/ai-edit", &e);
                                let err = json!({ "error": e.to_string() });
                                send_json_response(
                                    socket,
                                    "500 Internal Server Error",
                                    &err.to_string(),
                                )
                                .await;
                            }
                        }
                    } else {
                        send_json_response(
                            socket,
                            "400 Bad Request",
                            "{\"error\":\"invalid ai-edit request body\"}",
                        )
                        .await;
                    }
                }
                _ => {
                    send_json_response(
                        socket,
                        "404 Not Found",
                        "{\"error\":\"unknown video route\"}",
                    )
                    .await;
                }
            }
        }
        (_, route) if route.starts_with("/api/speech/") && method == "POST" => {
            // ── Speech Routes ───────────────────────────────────────────────────
            match route {
                "/api/speech/transcribe" => {
                    if let Ok(req) = serde_json::from_str::<TranscribeRequest>(body) {
                        let config = load_config().unwrap_or_default();
                        match transcribe(&config, &req).await {
                            Ok(res) => {
                                let json_str = serde_json::to_string(&res).unwrap_or_default();
                                log_api_req("POST", "/api/speech/transcribe", "200 OK", None);
                                send_json_response(socket, "200 OK", &json_str).await;
                            }
                            Err(e) => {
                                log_api_err("/api/speech/transcribe", &e);
                                let err = json!({ "error": e.to_string() });
                                send_json_response(
                                    socket,
                                    "500 Internal Server Error",
                                    &err.to_string(),
                                )
                                .await;
                            }
                        }
                    } else {
                        send_json_response(
                            socket,
                            "400 Bad Request",
                            "{\"error\":\"invalid transcribe request body\"}",
                        )
                        .await;
                    }
                }
                "/api/speech/detect-silence" => {
                    if let Ok(req) = serde_json::from_str::<DetectSilenceRequest>(body) {
                        match detect_silence(&req) {
                            Ok(ranges) => {
                                let json_str = serde_json::to_string(&ranges).unwrap_or_default();
                                log_api_req(
                                    "POST",
                                    "/api/speech/detect-silence",
                                    "200 OK",
                                    Some(&format!("{} ranges", ranges.len())),
                                );
                                send_json_response(socket, "200 OK", &json_str).await;
                            }
                            Err(e) => {
                                log_api_err("/api/speech/detect-silence", &e);
                                let err = json!({ "error": e.to_string() });
                                send_json_response(
                                    socket,
                                    "500 Internal Server Error",
                                    &err.to_string(),
                                )
                                .await;
                            }
                        }
                    } else {
                        send_json_response(
                            socket,
                            "400 Bad Request",
                            "{\"error\":\"invalid detect-silence request body\"}",
                        )
                        .await;
                    }
                }
                _ => {
                    send_json_response(
                        socket,
                        "404 Not Found",
                        "{\"error\":\"unknown speech route\"}",
                    )
                    .await;
                }
            }
        }
        (_, route) if route.starts_with("/api/subtitle/") && method == "POST" => {
            // ── Subtitle Routes ────────────────────────────────────────────────
            match route {
                "/api/subtitle/generate" => {
                    #[derive(serde::Deserialize)]
                    struct GenSubReq {
                        segments: Vec<crate::speech::TranscriptSegment>,
                    }
                    if let Ok(req) = serde_json::from_str::<GenSubReq>(body) {
                        let srt = generate_srt(&req.segments);
                        let res = json!({ "srt": srt });
                        log_api_req("POST", "/api/subtitle/generate", "200 OK", None);
                        send_json_response(socket, "200 OK", &res.to_string()).await;
                    } else {
                        send_json_response(
                            socket,
                            "400 Bad Request",
                            "{\"error\":\"invalid subtitle generate request body\"}",
                        )
                        .await;
                    }
                }
                "/api/subtitle/translate" => {
                    if let Ok(req) = serde_json::from_str::<TranslateSubtitleRequest>(body) {
                        let config = load_config().unwrap_or_default();
                        match translate_subtitles(&config, &req).await {
                            Ok(srt) => {
                                let res = json!({ "srt": srt });
                                log_api_req("POST", "/api/subtitle/translate", "200 OK", None);
                                send_json_response(socket, "200 OK", &res.to_string()).await;
                            }
                            Err(e) => {
                                log_api_err("/api/subtitle/translate", &e);
                                let err = json!({ "error": e.to_string() });
                                send_json_response(
                                    socket,
                                    "500 Internal Server Error",
                                    &err.to_string(),
                                )
                                .await;
                            }
                        }
                    } else {
                        send_json_response(
                            socket,
                            "400 Bad Request",
                            "{\"error\":\"invalid subtitle translate request body\"}",
                        )
                        .await;
                    }
                }
                "/api/subtitle/burn" => {
                    if let Ok(req) = serde_json::from_str::<BurnSubtitleRequest>(body) {
                        match burn_subtitles(&req) {
                            Ok(res) => {
                                let json_str = serde_json::to_string(&res).unwrap_or_default();
                                log_api_req("POST", "/api/subtitle/burn", "200 OK", None);
                                send_json_response(socket, "200 OK", &json_str).await;
                            }
                            Err(e) => {
                                log_api_err("/api/subtitle/burn", &e);
                                let err = json!({ "error": e.to_string() });
                                send_json_response(
                                    socket,
                                    "500 Internal Server Error",
                                    &err.to_string(),
                                )
                                .await;
                            }
                        }
                    } else {
                        send_json_response(
                            socket,
                            "400 Bad Request",
                            "{\"error\":\"invalid subtitle burn request body\"}",
                        )
                        .await;
                    }
                }
                _ => {
                    send_json_response(
                        socket,
                        "404 Not Found",
                        "{\"error\":\"unknown subtitle route\"}",
                    )
                    .await;
                }
            }
        }
        (_, "/api/video-gen/providers")
        | (_, "/api/video/providers")
        | ("GET", "/api/image-gen/providers") => {
            let config = load_config().unwrap_or_default();
            let mut available: Vec<String> = Vec::new();
            if !config.api_key.trim().is_empty() {
                available.push("nanobanana".into());
            }
            if !config.openai_api_key.trim().is_empty() {
                available.push("dalle".into());
            }
            if !config.stability_api_key.trim().is_empty() {
                available.push("stability".into());
            }
            if !config.ideogram_api_key.trim().is_empty() {
                available.push("ideogram".into());
            }
            if !config.replicate_api_key.trim().is_empty() {
                available.push("replicate".into());
            }
            if !config.bfl_api_key.trim().is_empty() {
                available.push("bfl".into());
            }
            let active = if available.contains(&config.image_gen_provider) {
                config.image_gen_provider.clone()
            } else {
                available
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "nanobanana".into())
            };
            let response = json!({ "active": active, "available": available });
            send_json_response(socket, "200 OK", &response.to_string()).await;
        }

        _ => unreachable!(
            "api_server routed an unhandled route into routes::media_gen::execute: {method} {route}"
        ),
    }
}

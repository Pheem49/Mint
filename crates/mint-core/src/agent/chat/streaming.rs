use futures_util::StreamExt;
use serde_json::Value;

use super::{ProviderReply, anthropic_input_tokens, decode_tool_arguments, extract_think_tag};
use crate::{ChatError, ChatStreamEvent, ToolCall};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StreamFormat {
    Gemini,
    OpenAi,
    Ollama,
    Anthropic,
}

#[derive(Default)]
pub(super) struct StreamToolCall {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) arguments: String,
    pub(super) input: Option<Value>,
    pub(super) thought_signature: Option<String>,
}

#[derive(Default)]
pub(super) struct StreamReplyAccumulator {
    pub(super) text: String,
    pub(super) thought: String,
    pub(super) tag_buffer: String,
    pub(super) active_think_close_tag: Option<&'static str>,
    pub(super) tool_calls: Vec<StreamToolCall>,
    pub(super) stop_reason: Option<String>,
    pub(super) input_tokens: Option<u32>,
    pub(super) output_tokens: Option<u32>,
    pub(super) anthropic_blocks: std::collections::HashMap<usize, usize>,
}

impl StreamReplyAccumulator {
    pub(super) fn tool_at(&mut self, index: usize) -> &mut StreamToolCall {
        while self.tool_calls.len() <= index {
            self.tool_calls.push(StreamToolCall::default());
        }
        &mut self.tool_calls[index]
    }

    pub(super) fn emit_text<F>(&mut self, delta: &str, on_event: &mut F)
    where
        F: FnMut(ChatStreamEvent),
    {
        if !delta.is_empty() {
            on_event(ChatStreamEvent::TextDelta {
                delta: delta.to_owned(),
            });
        }
    }

    /// Route inline `<think>` or `<thought>` content without assuming tag boundaries line up
    /// with provider chunks. A possible partial tag is retained until the next
    /// chunk; ordinary text is emitted immediately.
    pub(super) fn push_text<F>(&mut self, delta: &str, on_event: &mut F)
    where
        F: FnMut(ChatStreamEvent),
    {
        if delta.is_empty() {
            return;
        }
        self.text.push_str(delta);
        self.tag_buffer.push_str(delta);
        loop {
            if let Some(close_tag) = self.active_think_close_tag {
                if let Some(index) = self.tag_buffer.find(close_tag) {
                    let before = self.tag_buffer[..index].to_owned();
                    self.tag_buffer.drain(..index + close_tag.len());
                    self.push_reasoning(&before, on_event);
                    self.active_think_close_tag = None;
                    continue;
                }
                let retain_from = self.tag_buffer.rfind('<').and_then(|index| {
                    close_tag
                        .starts_with(&self.tag_buffer[index..])
                        .then_some(index)
                });
                let emit_end = retain_from.unwrap_or(self.tag_buffer.len());
                if emit_end > 0 {
                    let ready = self.tag_buffer[..emit_end].to_owned();
                    self.tag_buffer.drain(..emit_end);
                    self.push_reasoning(&ready, on_event);
                }
                break;
            } else {
                let candidates = [("<think>", "</think>"), ("<thought>", "</thought>")];
                let earliest = candidates
                    .into_iter()
                    .filter_map(|(open, close)| {
                        self.tag_buffer.find(open).map(|idx| (idx, open, close))
                    })
                    .min_by_key(|(idx, _, _)| *idx);
                if let Some((index, open_tag, close_tag)) = earliest {
                    let before = self.tag_buffer[..index].to_owned();
                    self.tag_buffer.drain(..index + open_tag.len());
                    self.emit_text(&before, on_event);
                    self.active_think_close_tag = Some(close_tag);
                    continue;
                }
                let retain_from = self.tag_buffer.rfind('<').and_then(|index| {
                    let tail = &self.tag_buffer[index..];
                    ("<think>".starts_with(tail) || "<thought>".starts_with(tail)).then_some(index)
                });
                let emit_end = retain_from.unwrap_or(self.tag_buffer.len());
                if emit_end > 0 {
                    let ready = self.tag_buffer[..emit_end].to_owned();
                    self.tag_buffer.drain(..emit_end);
                    self.emit_text(&ready, on_event);
                }
                break;
            }
        }
    }

    pub(super) fn flush_tag_buffer<F>(&mut self, on_event: &mut F)
    where
        F: FnMut(ChatStreamEvent),
    {
        if self.tag_buffer.is_empty() {
            return;
        }
        let remaining = std::mem::take(&mut self.tag_buffer);
        if self.active_think_close_tag.is_some() {
            self.push_reasoning(&remaining, on_event);
        } else {
            self.emit_text(&remaining, on_event);
        }
    }

    pub(super) fn push_reasoning<F>(&mut self, delta: &str, on_event: &mut F)
    where
        F: FnMut(ChatStreamEvent),
    {
        if !delta.is_empty() {
            self.thought.push_str(delta);
            on_event(ChatStreamEvent::ReasoningDelta {
                delta: delta.to_owned(),
            });
        }
    }

    pub(super) fn consume<F>(&mut self, format: StreamFormat, value: &Value, on_event: &mut F)
    where
        F: FnMut(ChatStreamEvent),
    {
        match format {
            StreamFormat::OpenAi => self.consume_openai(value, on_event),
            StreamFormat::Gemini => self.consume_gemini(value, on_event),
            StreamFormat::Ollama => self.consume_ollama(value, on_event),
            StreamFormat::Anthropic => self.consume_anthropic(value, on_event),
        }
    }

    pub(super) fn consume_openai<F>(&mut self, value: &Value, on_event: &mut F)
    where
        F: FnMut(ChatStreamEvent),
    {
        let choice = &value["choices"][0];
        let delta = &choice["delta"];
        if let Some(text) = delta["content"].as_str() {
            self.push_text(text, on_event);
        }
        if let Some(reasoning) = delta["reasoning_content"]
            .as_str()
            .or_else(|| delta["reasoning"].as_str())
            .or_else(|| delta["thinking"].as_str())
        {
            self.push_reasoning(reasoning, on_event);
        }
        if let Some(details) = delta["reasoning_details"].as_array() {
            for detail in details {
                if let Some(reasoning) = detail["text"].as_str() {
                    self.push_reasoning(reasoning, on_event);
                }
            }
        }
        if let Some(calls) = delta["tool_calls"].as_array() {
            for (fallback_index, call) in calls.iter().enumerate() {
                let index = call["index"]
                    .as_u64()
                    .map(|n| n as usize)
                    .unwrap_or(fallback_index);
                let name_delta = call["function"]["name"].as_str().map(str::to_owned);
                let arguments = call["function"]["arguments"].as_str().map(str::to_owned);
                let current_name = {
                    let tool = self.tool_at(index);
                    if let Some(id) = call["id"].as_str() {
                        tool.id.push_str(id);
                    }
                    if let Some(name) = &name_delta {
                        tool.name.push_str(name);
                    }
                    if let Some(arguments) = &arguments {
                        tool.arguments.push_str(arguments);
                    }
                    tool.name.clone()
                };
                if name_delta.is_some() || arguments.is_some() {
                    on_event(ChatStreamEvent::ToolCallDelta {
                        index,
                        name: name_delta.map(|_| current_name),
                        arguments,
                        input: None,
                    });
                }
            }
        }
        if let Some(reason) = choice["finish_reason"].as_str() {
            self.stop_reason = Some(reason.to_owned());
        }
        self.input_tokens = value["usage"]["prompt_tokens"]
            .as_u64()
            .map(|n| n as u32)
            .or(self.input_tokens);
        self.output_tokens = value["usage"]["completion_tokens"]
            .as_u64()
            .map(|n| n as u32)
            .or(self.output_tokens);
    }

    pub(super) fn consume_gemini<F>(&mut self, value: &Value, on_event: &mut F)
    where
        F: FnMut(ChatStreamEvent),
    {
        if let Some(parts) = value["candidates"][0]["content"]["parts"].as_array() {
            for (index, part) in parts.iter().enumerate() {
                if let Some(text) = part["text"].as_str() {
                    if part["thought"].as_bool().unwrap_or(false) {
                        self.push_reasoning(text, on_event);
                    } else {
                        self.push_text(text, on_event);
                    }
                }
                if let Some(call) = part.get("functionCall") {
                    let tool = self.tool_at(index);
                    tool.id = format!("call_{index}");
                    tool.name = call["name"].as_str().unwrap_or_default().to_owned();
                    tool.input = Some(call["args"].clone());
                    tool.thought_signature = part["thoughtSignature"].as_str().map(str::to_owned);
                    on_event(ChatStreamEvent::ToolCallDelta {
                        index,
                        name: call["name"].as_str().map(str::to_owned),
                        arguments: None,
                        input: Some(call["args"].clone()),
                    });
                }
            }
        }
        self.stop_reason = value["candidates"][0]["finishReason"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| self.stop_reason.take());
        self.input_tokens = value["usageMetadata"]["promptTokenCount"]
            .as_u64()
            .map(|n| n as u32)
            .or(self.input_tokens);
        self.output_tokens = value["usageMetadata"]["candidatesTokenCount"]
            .as_u64()
            .map(|n| n as u32)
            .or(self.output_tokens);
    }

    pub(super) fn consume_ollama<F>(&mut self, value: &Value, on_event: &mut F)
    where
        F: FnMut(ChatStreamEvent),
    {
        let message = &value["message"];
        if let Some(text) = message["content"].as_str() {
            self.push_text(text, on_event);
        }
        if let Some(reasoning) = message["thinking"]
            .as_str()
            .or_else(|| message["reasoning_content"].as_str())
        {
            self.push_reasoning(reasoning, on_event);
        }
        if let Some(calls) = message["tool_calls"].as_array() {
            for (index, call) in calls.iter().enumerate() {
                let tool = self.tool_at(index);
                tool.id = call["id"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("call_{index}"));
                tool.name = call["function"]["name"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned();
                tool.input = Some(call["function"]["arguments"].clone());
                on_event(ChatStreamEvent::ToolCallDelta {
                    index,
                    name: call["function"]["name"].as_str().map(str::to_owned),
                    arguments: None,
                    input: Some(call["function"]["arguments"].clone()),
                });
            }
        }
        self.input_tokens = value["prompt_eval_count"]
            .as_u64()
            .map(|n| n as u32)
            .or(self.input_tokens);
        self.output_tokens = value["eval_count"]
            .as_u64()
            .map(|n| n as u32)
            .or(self.output_tokens);
        if value["done"].as_bool().unwrap_or(false) {
            self.stop_reason = value["done_reason"]
                .as_str()
                .map(str::to_owned)
                .or_else(|| Some("stop".to_owned()));
        }
    }

    pub(super) fn consume_anthropic<F>(&mut self, value: &Value, on_event: &mut F)
    where
        F: FnMut(ChatStreamEvent),
    {
        match value["type"].as_str() {
            Some("message_start") => {
                self.input_tokens = anthropic_input_tokens(&value["message"]["usage"]);
            }
            Some("content_block_start") => {
                let block_index = value["index"].as_u64().unwrap_or(0) as usize;
                let block = &value["content_block"];
                if block["type"].as_str() == Some("tool_use") {
                    let tool_index = self.tool_calls.len();
                    let tool = self.tool_at(tool_index);
                    tool.id = block["id"].as_str().unwrap_or_default().to_owned();
                    tool.name = block["name"].as_str().unwrap_or_default().to_owned();
                    self.anthropic_blocks.insert(block_index, tool_index);
                    on_event(ChatStreamEvent::ToolCallDelta {
                        index: tool_index,
                        name: block["name"].as_str().map(str::to_owned),
                        arguments: None,
                        input: None,
                    });
                }
            }
            Some("content_block_delta") => {
                let delta = &value["delta"];
                match delta["type"].as_str() {
                    Some("text_delta") => {
                        if let Some(text) = delta["text"].as_str() {
                            self.push_text(text, on_event);
                        }
                    }
                    Some("thinking_delta") => {
                        if let Some(thinking) = delta["thinking"].as_str() {
                            self.push_reasoning(thinking, on_event);
                        }
                    }
                    Some("input_json_delta") => {
                        let block_index = value["index"].as_u64().unwrap_or(0) as usize;
                        if let Some(tool_index) = self.anthropic_blocks.get(&block_index).copied()
                            && let Some(json) = delta["partial_json"].as_str()
                        {
                            self.tool_at(tool_index).arguments.push_str(json);
                            on_event(ChatStreamEvent::ToolCallDelta {
                                index: tool_index,
                                name: None,
                                arguments: Some(json.to_owned()),
                                input: None,
                            });
                        }
                    }
                    _ => {}
                }
            }
            Some("message_delta") => {
                self.stop_reason = value["delta"]["stop_reason"]
                    .as_str()
                    .map(str::to_owned)
                    .or_else(|| self.stop_reason.take());
                self.output_tokens = value["usage"]["output_tokens"]
                    .as_u64()
                    .map(|n| n as u32)
                    .or(self.output_tokens);
            }
            _ => {}
        }
    }

    pub(super) fn finish(self, model: String) -> Result<ProviderReply, ChatError> {
        let (tag_thought, clean_text) = extract_think_tag(&self.text);
        let thought = if self.thought.trim().is_empty() {
            tag_thought
        } else {
            Some(self.thought.trim().to_owned())
        };
        let calls: Vec<ToolCall> = self
            .tool_calls
            .into_iter()
            .filter(|tool| !tool.name.is_empty())
            .map(|tool| {
                let input = tool
                    .input
                    .unwrap_or_else(|| decode_tool_arguments(&Value::String(tool.arguments)));
                ToolCall {
                    id: tool.id,
                    name: tool.name,
                    input,
                    thought_signature: tool.thought_signature,
                }
            })
            .collect();
        if clean_text.is_empty() && calls.is_empty() && thought.is_none() {
            return Err(ChatError::MissingResponseText);
        }
        Ok(ProviderReply {
            model,
            text: if clean_text.is_empty() && calls.is_empty() {
                thought.clone().unwrap_or_default()
            } else {
                clean_text
            },
            thought,
            tool_calls: (!calls.is_empty()).then_some(calls),
            stop_reason: self.stop_reason,
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
        })
    }
}

pub(super) async fn collect_stream_reply<F>(
    response: reqwest::Response,
    format: StreamFormat,
    model: String,
    on_event: &mut F,
) -> Result<ProviderReply, ChatError>
where
    F: FnMut(ChatStreamEvent),
{
    let mut bytes = response.bytes_stream();
    let mut buffer = Vec::new();
    let mut accumulator = StreamReplyAccumulator::default();
    while let Some(chunk) = bytes.next().await {
        buffer.extend_from_slice(&chunk?);
        while let Some(index) = buffer.iter().position(|&b| b == b'\n') {
            let line = String::from_utf8_lossy(&buffer[..index]).trim().to_owned();
            buffer.drain(..=index);
            if let Some(value) = parse_stream_value(format, &line) {
                accumulator.consume(format, &value, on_event);
            }
        }
    }
    if !buffer.is_empty() {
        let trailing = String::from_utf8_lossy(&buffer);
        if let Some(value) = parse_stream_value(format, trailing.trim()) {
            accumulator.consume(format, &value, on_event);
        }
    }
    accumulator.flush_tag_buffer(on_event);
    accumulator.finish(model)
}

pub(super) fn parse_stream_value(format: StreamFormat, line: &str) -> Option<Value> {
    let payload = match format {
        StreamFormat::Ollama => line,
        _ => line.strip_prefix("data:")?.trim(),
    };
    if payload.is_empty() || payload == "[DONE]" {
        return None;
    }
    serde_json::from_str(payload).ok()
}

use std::{future::Future, pin::Pin};

use serde::de::DeserializeOwned;
use thiserror::Error;

use crate::{ChatError, ChatRequest, MintConfig, send_chat};

pub type AgentActionFuture = Pin<Box<dyn Future<Output = Result<String, String>> + Send>>;

#[derive(Debug, Error)]
pub enum AgentLoopError {
    #[error(transparent)]
    Chat(#[from] ChatError),
    #[error("provider did not return a valid agent action: {0}")]
    InvalidAction(String),
    #[error("agent action failed: {0}")]
    Action(String),
    #[error("agent reached the limit of {0} steps")]
    StepLimit(usize),
}

pub fn parse_agent_json<T: DeserializeOwned>(raw: &str) -> Result<T, AgentLoopError> {
    serde_json::from_str(raw).or_else(|_| {
        let start = raw
            .find('{')
            .ok_or_else(|| AgentLoopError::InvalidAction("missing JSON object".into()))?;
        let end = raw
            .rfind('}')
            .ok_or_else(|| AgentLoopError::InvalidAction("missing JSON object".into()))?;
        serde_json::from_str(&raw[start..=end])
            .map_err(|error| AgentLoopError::InvalidAction(error.to_string()))
    })
}

pub async fn run_agent_loop<A, Parse, Done, Execute, Observe>(
    config: &MintConfig,
    system_instruction: &str,
    initial_observation: String,
    max_steps: usize,
    mut parse: Parse,
    mut done: Done,
    mut execute: Execute,
    mut observe: Observe,
) -> Result<String, AgentLoopError>
where
    A: Send + 'static,
    Parse: FnMut(&str) -> Result<A, String>,
    Done: FnMut(&A) -> Option<String>,
    Execute: FnMut(usize, A) -> AgentActionFuture,
    Observe: FnMut(usize, &A) -> Result<(), String>,
{
    let mut observation = initial_observation;
    for step in 1..=max_steps {
        let response = send_chat(
            config,
            &ChatRequest {
                message: observation,
                system_instruction: system_instruction.into(),
                image_data_uri: None,
                audio_data_uri: None,
                document_attachment: None,
            },
        )
        .await?;
        let action = parse(&response.text).map_err(AgentLoopError::InvalidAction)?;
        observe(step, &action).map_err(AgentLoopError::Action)?;
        if let Some(result) = done(&action) {
            return Ok(result);
        }
        observation = execute(step, action)
            .await
            .map_err(AgentLoopError::Action)?;
    }
    Err(AgentLoopError::StepLimit(max_steps))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Action {
        action: String,
    }

    #[test]
    fn parses_json_wrapped_in_provider_text() {
        let action: Action = parse_agent_json("```json\n{\"action\":\"done\"}\n```").unwrap();
        assert_eq!(action.action, "done");
    }
}

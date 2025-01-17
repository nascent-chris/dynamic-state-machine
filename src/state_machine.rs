use anyhow::Context as _;
use regex::Regex;
use serde::Deserialize;
use tokio::sync::broadcast;

use std::env;
use std::future::Future;
use std::{borrow::Cow, time::Duration};
use tracing::Instrument as _;

use crate::config::{Action, ActionDiscriminants, Config};
use crate::models::CallApiData;

pub struct StateMachine {
    config: Config,
    current_state_key: String,
    input_tx: broadcast::Sender<String>,
}

impl StateMachine {
    pub async fn new(config_path: &str) -> Result<Self, anyhow::Error> {
        let config = Self::load_config(config_path)
            .await
            .with_context(|| format!("failed to load config from {}", config_path))?;
        let current_state_key = config.initial_state_key.clone();
        let (input_tx, _) = broadcast::channel(100);
        Ok(Self {
            config,
            current_state_key,
            input_tx,
        })
    }

    async fn load_config(path: &str) -> Result<Config, anyhow::Error> {
        let data = tokio::fs::read_to_string(path).await?;
        let config = serde_json::from_str(&data)?;
        Ok(config)
    }

    pub fn run(self) -> impl Future<Output = Result<String, anyhow::Error>> + Send {
        tracing::info!("starting state machine");
        let mut next_state_key = self.current_state_key.clone();

        async move {
            let mut response_buffer = Some("intitial input!".to_string());
            while let Some(state_config) = self.config.states.get(&next_state_key) {
                tracing::info!(state_key = %next_state_key, "executing state");
                for action in &state_config.action {
                    let action_discriminant = ActionDiscriminants::from(action);
                    response_buffer = self
                        .execute_action(action.clone(), response_buffer.as_ref())
                        .instrument(tracing::debug_span!("action", action = ?action_discriminant))
                        .await?;
                }

                if let Some(next_state) = &state_config.next_state {
                    tracing::debug!(
                        state_key = %next_state_key,
                        next_state = %next_state,
                        "exiting state"
                    );
                    next_state_key = next_state.clone();
                } else {
                    tracing::info!(
                        state_key = %next_state_key,
                        "no next state. State machine is terminating."
                    );
                    break;
                }
            }

            Ok(response_buffer.unwrap_or_default())
        }
    }

    pub async fn execute_action(
        &self,
        action: Action,
        response_buffer: Option<&String>,
    ) -> Result<Option<String>, anyhow::Error> {
        match action {
            Action::CallApi(_call_api_data) => {
                let response = self.call_api_data(&_call_api_data).await?;
                Ok(Some(response))
            }

            Action::Llm(llm_data) => {
                let user_prompt = StateMachine::process_placeholders(
                    &llm_data.user_prompt,
                    response_buffer.as_ref().map(|s| s.as_str()),
                )?;
                let system_prompt = llm_data.system_prompt.and_then(|s| {
                    StateMachine::process_placeholders(&s, response_buffer.map(|s| s.as_str()))
                        .ok()
                        .map(|s| s.to_string())
                });

                tracing::info!(%user_prompt, ?system_prompt, "processed prompt");
                // Use `prompt` with the LLM
                Ok(None)
            }
            Action::SpawnAgent(agent_data) => {
                tracing::info!(?agent_data, "spawning agent");
                let config_file = agent_data.agent_config_file.clone();

                tokio::spawn(async move {
                    let agent_state_machine = StateMachine::new(&config_file).await?;
                    agent_state_machine.run().await
                })
                .await??;
                Ok(None)
            }
            Action::WaitForInput => {
                let mut input_rx = self.input_tx.subscribe();
                match tokio::time::timeout(Duration::from_secs(10), input_rx.recv()).await {
                    Ok(Ok(input)) => {
                        tracing::info!(input = %input, "received input");
                        Ok(Some(input))
                    }
                    Ok(Err(broadcast::error::RecvError::Closed)) => {
                        tracing::info!("input channel closed");
                        Ok(None)
                    }
                    Ok(Err(broadcast::error::RecvError::Lagged(n))) => {
                        tracing::info!(n = %n, "missed messages");
                        Ok(None)
                    }
                    Err(_) => {
                        tracing::warn!("receive timed out after 10 seconds");
                        Ok(None)
                    }
                }
            }
        }
    }

    async fn call_api_data(&self, call_api_data: &CallApiData) -> Result<String, anyhow::Error> {
        let client = reqwest::Client::new();
        let response = client
            .request((&call_api_data.method).into(), &call_api_data.url)
            .header(
                call_api_data.auth_header_name.as_str(),
                call_api_data.auth_header_value.clone(),
            )
            .body(call_api_data.body.clone().unwrap_or_default())
            .send()
            .await?;
        Ok(response.text().await?)
    }

    fn process_placeholders<'a>(
        template: &'a str,
        response_buffer: Option<&str>,
    ) -> Result<Cow<'a, str>, anyhow::Error> {
        // matching for all placeholders in the form {identifier:value}
        let re = Regex::new(r"\{([^}]+)\}")?;

        Ok(re.replace_all(template, |caps: &regex::Captures| {
            let placeholder_text = &caps[1];
            let mut parts = placeholder_text.splitn(2, ':');
            let identifier = parts.next().unwrap_or("");
            let value = parts.next().unwrap_or("");

            serde_plain::from_str::<Placeholder>(identifier)
                .map(|placeholder| match placeholder {
                    Placeholder::Input => response_buffer
                        .map_or(Cow::Borrowed(""), Cow::Borrowed),
                    Placeholder::Env => env::var(value)
                        .map_or(Cow::Borrowed(""), Cow::Owned),
                })
                .unwrap_or_else(|err| {
                    tracing::error!(error = %err, identifier = %identifier, "error parsing placeholder");
                    Cow::Borrowed("")
                })
        }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Placeholder {
    Input,
    Env,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_placeholders() {
        // set an environment variable
        env::set_var("TEST_VAR", "Hello");
        let input = "world!".to_string();
        let result =
            StateMachine::process_placeholders("{env:TEST_VAR} {input}", Some(&input)).unwrap();
        println!("{}", result);
        assert_eq!(result, "Hello world!");
    }
}

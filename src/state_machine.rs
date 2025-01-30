use anyhow::Context as _;
use futures::Stream;
use regex::Regex;
use serde::Deserialize;
use tokio::sync::{broadcast, mpsc};

use std::collections::HashMap;
use std::env;
use std::future::Future;
use std::ops::Not;
use std::{borrow::Cow, time::Duration};
use tracing::Instrument as _;

use crate::config::{Action, ActionDiscriminants, Config};
use crate::models::{AgentConfigSource, CallApiData};

pub struct StateMachine {
    config: Config,
    current_state_key: String,
    input_rx: Option<broadcast::Receiver<String>>,
    output_tx: Option<broadcast::Sender<String>>,
    config_update_tx: mpsc::Sender<Config>,
    config_update_rx: mpsc::Receiver<Config>,
    streams_map: HashMap<String, broadcast::Sender<String>>,
}

impl StateMachine {
    pub async fn new(config_path: &str) -> Result<Self, anyhow::Error> {
        let config = Self::load_config_from_path(config_path)
            .await
            .with_context(|| format!("failed to load config from {}", config_path))?;
        let current_state_key = config.initial_state_key.clone();

        let (config_update_tx, config_update_rx) = mpsc::channel(100);

        let mut streams_map = HashMap::new();

        // for every SpawnAgent action, create a new stream and add it to the streams_map
        for action in config
            .states
            .values()
            .flat_map(|state| state.actions.iter())
        {
            if let Action::SpawnAgent { agent_data } = action {
                let (tx, _) = broadcast::channel(100);

                streams_map.insert(agent_data.output_label.clone(), tx);
            }
        }

        Ok(Self {
            config,
            current_state_key,
            input_rx: None,
            output_tx: None,
            config_update_tx,
            config_update_rx,
            streams_map,
        })
    }

    async fn load_config_from_path(path: &str) -> Result<Config, anyhow::Error> {
        let data = tokio::fs::read_to_string(path).await?;
        let config = serde_json::from_str(&data)?;
        Ok(config)
    }

    pub fn get_config_update_tx(&self) -> mpsc::Sender<Config> {
        self.config_update_tx.clone()
    }

    pub fn run(mut self) -> impl Future<Output = Result<Vec<String>, anyhow::Error>> + Send {
        tracing::info!("starting state machine");
        let mut next_state_key = self.current_state_key.clone();

        async move {
            let mut response_buffer = Vec::new();
            while let Some(state_config) = self.config.states.get(&next_state_key) {
                tracing::info!(state_key = %next_state_key, "executing state");

                // Collect futures for all actions
                let action_futures = state_config.actions.iter().map(|action| {
                    let action_discriminant = ActionDiscriminants::from(action);
                    self.execute_action(action, &response_buffer)
                        .instrument(tracing::debug_span!("action", action = ?action_discriminant))
                });

                // Execute all actions in parallel
                let results = futures::future::join_all(action_futures).await;

                // Process and collect responses, replacing response_buffer
                response_buffer = results
                    .into_iter()
                    .inspect(|result| {
                        if let Err(e) = result {
                            tracing::error!(error = %e, "action failed");
                        }
                    })
                    .flatten()
                    .flatten()
                    .map(|result| {
                        StateMachine::process_placeholders(&result, response_buffer.first())
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                // Process next state
                if let Some(next_state_template) = &state_config.next_state {
                    // Process placeholders in next_state
                    let processed_next_state = StateMachine::process_placeholders(
                        next_state_template,
                        response_buffer.first(),
                    )?;
                    tracing::debug!(
                        state_key = %next_state_key,
                        next_state = %processed_next_state,
                        "exiting state"
                    );
                    if self.config.states.contains_key(&processed_next_state) {
                        next_state_key = processed_next_state;
                    } else {
                        tracing::error!(
                            state_key = %next_state_key,
                            next_state = %processed_next_state,
                            "next state not found"
                        );
                        break;
                    }
                } else {
                    tracing::info!(
                        state_key = %next_state_key,
                        response_buffer = ?response_buffer,
                        "no next state. State machine is returning."
                    );
                    break;
                }

                // Check for config updates
                if let Ok(config) = self.config_update_rx.try_recv() {
                    self.config = config;
                    self.current_state_key = self.config.initial_state_key.clone();
                    tracing::info!(
                        initial_state_key = %self.current_state_key,
                        "config updated, restarting state machine"
                    );
                }
            }

            Ok(response_buffer)
        }
    }

    pub async fn execute_action(
        &self,
        action: &Action,
        response_buffer: &[String],
    ) -> Result<Option<String>, anyhow::Error> {
        match action {
            Action::CallApi(call_api_data) => {
                let response = self.call_api_data(call_api_data).await?;
                Ok(Some(response))
            }
            Action::Llm(llm_data) => {
                let user_prompt = StateMachine::process_placeholders(
                    &llm_data.user_prompt,
                    response_buffer.first(),
                )?;
                let system_prompt = llm_data.system_prompt.as_ref().and_then(|s| {
                    StateMachine::process_placeholders(s, response_buffer.first())
                        .ok()
                        .map(|s| s.to_string())
                });

                tracing::info!(%user_prompt, ?system_prompt, "processed prompt");
                // Use `user_prompt` and `system_prompt` with the LLM
                Ok(None)
            }
            Action::SpawnAgent { agent_data } => {
                tracing::info!(?agent_data, "spawning agent");

                // Retrieve the agent's config
                let agent_config = match &agent_data.config_source {
                    AgentConfigSource::File { agent_config_file } => {
                        Self::load_config_from_path(agent_config_file)
                            .await
                            .with_context(|| {
                                format!("failed to load config from {}", agent_config_file)
                            })?
                    }
                    AgentConfigSource::Inline { agent_config } => agent_config.clone(),
                };

                let input_rx = self
                    .streams_map
                    .get(&agent_data.input_label)
                    .map(|tx| tx.subscribe());

                let output_tx = self
                    .streams_map
                    .get(&agent_data.output_label)
                    .map(|tx| tx.clone());

                let res = tokio::spawn(async move {
                    let mut agent_state_machine = StateMachine::new_with_config(agent_config);
                    agent_state_machine.input_rx = input_rx;
                    agent_state_machine.output_tx = output_tx;
                    agent_state_machine.run().await
                })
                .await??;

                tracing::debug!(?res, "agent result");

                Ok(Some(res.join("\n")))
            }
            Action::WaitForInput => {
                let Some(mut input_rx) = self.input_rx.as_ref().map(|rx| rx.resubscribe()) else {
                    tracing::error!("no input channel found");
                    return Ok(None);
                };
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
                        tracing::error!(n = %n, "missed messages");
                        Ok(None)
                    }
                    Err(_) => {
                        tracing::warn!("receive timed out after 10 seconds");
                        Ok(None)
                    }
                }
            }
            Action::Yield => {
                tracing::info!("yielding");
                if let Some(output_tx) = self.output_tx.as_ref() {
                    if let Some(response) = response_buffer.first() {
                        output_tx.send(response.clone())?;
                    }
                }
                Ok(None)
            }
            Action::GetAgentConfig(label) => {
                tracing::info!(label = %label, "getting agent config");
                Ok(None)
            }
            Action::SetAgentConfig(label) => {
                tracing::info!(label = %label, "setting agent config");
                Ok(None)
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

    fn process_placeholders(
        template: &str,
        response_buffer: Option<&String>,
    ) -> Result<String, anyhow::Error> {
        let re = Regex::new(r"\{([^}]+)\}")?;

        let result = re.replace_all(template, |caps: &regex::Captures| {
            let placeholder_text = &caps[1];

            // Deserialize placeholder_text into Placeholder enum
            let placeholder: Placeholder =
                match serde_json::from_str(&format!("\"{}\"", placeholder_text)) {
                    Ok(p) => p,
                    Err(_) => {
                        tracing::error!(placeholder = %placeholder_text, "Invalid placeholder");
                        return "".to_string();
                    }
                };

            match placeholder {
                Placeholder::Input => {
                    // For this example, we'll assume "Input" refers to the first element
                    response_buffer.cloned().unwrap_or_default()
                }
                Placeholder::Output => {
                    // "Output" refers to the last element in the response buffer
                    response_buffer.cloned().unwrap_or_default()
                }
                Placeholder::Env(var_name) => env::var(&var_name).unwrap_or_default(),
            }
        });

        Ok(result.into_owned())
    }

    pub fn new_with_config(config: Config) -> Self {
        let current_state_key = config.initial_state_key.clone();
        let (config_update_tx, config_update_rx) = mpsc::channel(100);
        Self {
            config,
            current_state_key,
            input_rx: None,
            output_tx: None,
            config_update_tx,
            config_update_rx,
            streams_map: HashMap::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Placeholder {
    Input,
    Output,
    Env(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Action;
    use serde_json::json;

    #[tokio::test]
    async fn test_run_parallel_actions() {
        // Mock configuration with multiple actions
        let config = Config {
            label: "test".to_string(),
            initial_state_key: "start".to_string(),
            states: vec![(
                "start".to_string(),
                crate::config::AgentConfig {
                    actions: vec![Action::WaitForInput],
                    next_state: None,
                },
            )]
            .into_iter()
            .collect(),
            output_stream: None,
        };

        let (input_tx, _) = broadcast::channel(1);
        let (config_update_tx, config_update_rx) = mpsc::channel(1);
        let state_machine = StateMachine {
            config,
            current_state_key: "start".to_string(),
            input_rx: Some(input_tx.subscribe()),
            output_tx: None,
            config_update_tx,
            config_update_rx,
            streams_map: HashMap::new(),
        };

        //wait for 1ms to make sure the input_tx is ready
        tokio::time::sleep(Duration::from_millis(1)).await;
        input_tx.send("test".to_string()).unwrap();

        let responses = state_machine.run().await.unwrap();
        // Assert on responses
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0], "test");
    }
}

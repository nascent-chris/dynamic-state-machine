use regex::Regex;
use serde::Deserialize;
use serde_plain::from_str;
use std::env;
use std::future::Future;
use tracing::Instrument as _;

use crate::config::{Action, ActionDiscriminants, Config};

pub struct StateMachine {
    config: Config,
    current_state_key: String,
}

impl StateMachine {
    pub async fn new(config_path: &str) -> Result<Self, anyhow::Error> {
        let config = Self::load_config(config_path).await?;
        let current_state_key = config.initial_state_key.clone();
        Ok(Self {
            config,
            current_state_key,
        })
    }

    async fn load_config(path: &str) -> Result<Config, anyhow::Error> {
        let data = tokio::fs::read_to_string(path).await?;
        let config = serde_json::from_str(&data)?;
        Ok(config)
    }

    pub fn run(self) -> impl Future<Output = Result<(), anyhow::Error>> + Send {
        tracing::info!("starting state machine");
        let mut next_state_key = self.current_state_key.clone();

        let mut response_buffer = Some("intitial input!".to_string());

        async move {
            while let Some(state_config) = self.config.states.get(&next_state_key) {
                tracing::info!(state_key = %next_state_key, "executing state");
                for action in &state_config.action {
                    let action_discriminant = ActionDiscriminants::from(action);
                    self.execute_action(action.clone(), response_buffer.as_ref())
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

            Ok(())
        }
    }

    pub async fn execute_action(
        &self,
        action: Action,
        response_buffer: Option<&String>,
    ) -> Result<Option<String>, anyhow::Error> {
        match action {
            Action::CallApi(_call_api_data) => {
                // Handle API call
            }
            Action::ProcessResponse => {
                // Handle processing response
            }
            Action::Finalize => {
                // Finalize state machine
            }
            Action::Llm(llm_data) => {
                let prompt = StateMachine::process_placeholders(&llm_data.user_prompt, response_buffer)?;
                tracing::info!(%prompt, "processed prompt");
                // Use `prompt` with the LLM
            }
            Action::SpawnAgent(agent_data) => {
                tracing::info!(?agent_data, "spawning agent");
                let config_file = agent_data.agent_config_file.clone();

                tokio::spawn(async move {
                    let agent_state_machine = StateMachine::new(&config_file).await?;
                    agent_state_machine.run().await
                })
                .await??;
            }
            Action::WaitForInput => {
                // Handle waiting for input
            }
        }

        Ok(None)
    }

    fn process_placeholders(
        template: &str,
        response_buffer: Option<&String>,
    ) -> Result<String, anyhow::Error> {
            // Start of Selection
            let re = Regex::new(r"\{([^}]+)\}")?;
    
            let result = re.replace_all(template, |caps: &regex::Captures| {
                let placeholder_text = &caps[1];
                
                let mut parts = placeholder_text.splitn(2, ':');
                let identifier = parts.next().unwrap_or("");
                let value = parts.next().unwrap_or("");
    
                serde_plain::from_str::<Placeholder>(identifier)
                    .map(|placeholder| match placeholder {
                        Placeholder::Input => response_buffer
                            .map_or_else(String::new, |s| s.clone()),
                        Placeholder::Env => env::var(value).unwrap_or_default(),
                    })
                    .unwrap_or_else(|err| {
                        tracing::error!(error = %err, identifier = %identifier, "error parsing placeholder");
                        String::new()
                    })
            })
            .to_string();
    
            Ok(result)
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
        let result = StateMachine::process_placeholders("{env:TEST_VAR} {input}", Some(&input)).unwrap();
        println!("{}", result);
        assert_eq!(result, "Hello world!");
    }
}

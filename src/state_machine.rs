use tracing::Instrument;

use crate::config::{Action, Config};
use crate::models::{ApiResponse, ProcessedData};
use std::error::Error;

pub struct StateMachine {
    config: Config,
    current_state_key: String,
    response_buffer: Option<String>,
}

impl StateMachine {
    pub async fn new(config_path: &str) -> Result<Self, anyhow::Error> {
        let config = Self::load_config(config_path).await?;
        let current_state_key = config.initial_state_key.clone();
        Ok(Self {
            config,
            current_state_key,
            response_buffer: None,
        })
    }

    async fn load_config(path: &str) -> Result<Config, anyhow::Error> {
        let data = tokio::fs::read_to_string(path).await?;
        let config = serde_json::from_str(&data)?;
        Ok(config)
    }

    pub async fn run(&mut self) -> Result<(), anyhow::Error> {
        // Start of Selection
        while let Some(state_config) = self.config.states.get(&self.current_state_key) {
            tracing::debug!(state_key = self.current_state_key, "entering state");
            let state_config = state_config.clone();
            for action in state_config.action {
                self.execute_action(action.clone())
                    .instrument(tracing::debug_span!("action", action = ?action))
                    .await?;
            }

            if let Some(next_state) = state_config.next_state {
                tracing::debug!(
                    state_key = self.current_state_key,
                    next_state = next_state,
                    "exiting state"
                );
                self.current_state_key = next_state;
            } else {
                tracing::info!(
                    state_key = self.current_state_key,
                    "no next state. State machine is terminating."
                );
                break;
            }
        }
        Ok(())
    }

    pub async fn execute_action(&mut self, action: Action) -> Result<(), anyhow::Error> {
        let response_buffer = self.response_buffer.take();
        tracing::debug!(?action, "executing action");
        match action {
            Action::CallApi(_call_api_data) => {
                // Use call_api_data.url, call_api_data.auth_header_name, call_api_data.auth_header_value
                // Perform the API call without unnecessary cloning
                self.response_buffer = Some(String::from("API response"));
            }
            Action::ProcessResponse => {
                // Handle processing response
            }
            Action::Finalize => {
                // Finalize state machine
            }
            Action::Llm(llm_data) => {
                // Handle LLM processing
                // replace `{input}` in llm_data.prompt with response_buffer
                let prompt = if let Some(response_buffer) = response_buffer {
                    tracing::info!(?response_buffer, "using response buffer");
                    llm_data
                        .user_prompt
                        .replace("{input}", response_buffer.as_str())
                } else {
                    tracing::info!("no response buffer");
                    llm_data.user_prompt.clone()
                };

                tracing::info!("prompt: {}", prompt);
            }
        }
        Ok(())
    }

    // async fn call_api(&self) -> Result<ApiResponse, Box<dyn Error>> {
    //     // Implement the API call asynchronously
    //     Ok(ApiResponse::default())
    // }

    // async fn process_response(&self) -> Result<(), Box<dyn Error>> {
    //     // Implement your processing logic here
    //     Ok(())
    // }

    // async fn finalize(&self) -> Result<(), Box<dyn Error>> {
    //     // Implement your finalization logic here
    //     Ok(())
    // }
}

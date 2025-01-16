use crate::config::Config;
use crate::models::{ApiResponse, ProcessedData};
use std::error::Error;

pub struct StateMachine {
    config: Config,
    current_state: String,
}

impl StateMachine {
    pub async fn new(config_path: &str) -> Result<Self, Box<dyn Error>> {
        let config = Self::load_config(config_path).await?;
        let current_state = config.initial_state.clone();
        Ok(Self {
            config,
            current_state,
        })
    }

    async fn load_config(path: &str) -> Result<Config, Box<dyn Error>> {
        let data = tokio::fs::read_to_string(path).await?;
        let config = serde_json::from_str(&data)?;
        Ok(config)
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        while let Some(state_config) = self.config.states.get(&self.current_state) {
            tracing::debug!("Current state: {}", self.current_state);
            self.execute_action(&state_config.action).await?;

            if let Some(next_state) = &state_config.next_state {
                self.current_state = next_state.clone();
            } else {
                tracing::info!("No next state. State machine is terminating.");
                break;
            }
        }
        Ok(())
    }

    async fn execute_action(&self, action: &str) -> Result<(), Box<dyn Error>> {
        match action {
            "call_api" => {
                let response = self.call_api().await?;
                tracing::debug!("API Response: {:?}", response);
            }
            "process_response" => {
                let processed_data = self.process_response().await?;
                tracing::debug!("Processed Data: {:?}", processed_data);
            }
            "finalize" => {
                self.finalize().await?;
            }
            _ => {
                return Err(format!("Unknown action '{}'", action).into());
            }
        }
        Ok(())
    }

    async fn call_api(&self) -> Result<ApiResponse, Box<dyn Error>> {
        // Implement the API call asynchronously
        Ok(ApiResponse::default())
    }

    async fn process_response(&self) -> Result<ProcessedData, Box<dyn Error>> {
        // Process the API response
        Ok(ProcessedData::default())
    }

    async fn finalize(&self) -> Result<(), Box<dyn Error>> {
        // Finalize the process
        Ok(())
    }
}

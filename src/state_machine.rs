use std::future::Future;

use tracing::Instrument as _;

use crate::config::{Action, ActionDiscriminants, Config};
use crate::models::AgentData;

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

    pub fn run(self) -> impl Future<Output = anyhow::Result<()>> + Send + 'static {
        tracing::info!("starting state machine");
        let mut next_state_key = self.current_state_key.clone();

        async move {
            while let Some(state_config) = self.config.states.get(&next_state_key).cloned() {
                // tracing::debug!(state_key = next_state_key, "entering state");
                let state_config = state_config.clone();
                for action in state_config.action {
                    // let dummy_response_buffer = Some(String::from("API response"));
                    let action_discriminant = ActionDiscriminants::from(&action);
                    self.execute_action(action, None)
                        .instrument(tracing::debug_span!("action", action = ?action_discriminant))
                        .await?;
                }

                if let Some(next_state) = state_config.next_state {
                    tracing::debug!(
                        state_key = next_state_key,
                        next_state = next_state,
                        "exiting state"
                    );
                    next_state_key = next_state;
                } else {
                    tracing::info!(
                        state_key = next_state_key,
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
        response_buffer: Option<String>,
    ) -> Result<Option<String>, anyhow::Error> {
        match action {
            Action::CallApi(_call_api_data) => {
                //
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
                    tracing::debug!(?response_buffer, "using response buffer");
                    llm_data
                        .user_prompt
                        .replace("{input}", response_buffer.as_str())
                } else {
                    tracing::debug!("no response buffer");
                    llm_data.user_prompt.clone()
                };

                tracing::info!(?prompt, "prompt");
            }
            Action::SpawnAgent(agent_data) => {
                // Spawn a new agent
                tracing::info!(?agent_data, "spawning agent");

                spawn_new_agent(agent_data).await?;
            }
        }
        // TODO: return response buffer
        let response_buffer = None;
        Ok(response_buffer)
    }
}
async fn spawn_new_agent(agent_data: AgentData) -> Result<(), anyhow::Error> {
    tokio::spawn(async move {
        let agent_state_machine = StateMachine::new(&agent_data.agent_config_file).await?;
        agent_state_machine.run().await?;

        Ok::<(), anyhow::Error>(())
    })
    .await??;

    Ok(())
}

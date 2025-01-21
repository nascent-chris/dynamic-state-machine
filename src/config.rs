use crate::models::{AgentData, CallApiData, LlmData};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use strum_macros::EnumDiscriminants;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(alias = "initial_state")]
    pub initial_state_key: String,
    pub label: String,
    pub states: HashMap<String, AgentConfig>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AgentConfig {
    pub actions: Vec<Action>,
    pub next_state: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, EnumDiscriminants)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    CallApi(CallApiData),
    Llm(LlmData),
    SpawnAgent {
        #[serde(flatten)]
        agent_data: AgentData,
    },
    WaitForInput,
    GetAgentConfig(String),
    SetAgentConfig(String),
}

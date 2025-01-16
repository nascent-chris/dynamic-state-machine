use crate::models::{CallApiData, LlmData};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(alias = "initial_state")]
    pub initial_state_key: String,
    pub states: HashMap<String, StateConfig>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StateConfig {
    pub action: Vec<Action>,
    pub next_state: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
// Start of Selection
#[serde(rename_all = "snake_case")]
pub enum Action {
    CallApi(CallApiData),
    Llm(LlmData),
    ProcessResponse,
    Finalize,
}

// Start of Selection
// impl<'de> serde::Deserialize<'de> for Config {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: serde::Deserializer<'de>,
//     {
//         #[derive(Deserialize)]
//         struct ConfigHelper {
//             initial_state: String,
//             states: HashMap<String, StateConfig>,
//         }

//         let helper = ConfigHelper::deserialize(deserializer)?;

//         Ok(Config {
//             initial_state_key: helper.initial_state,
//             states: helper.states,
//         })
//     }
// }

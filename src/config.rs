use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub initial_state: String,
    pub states: HashMap<String, StateConfig>,
}

#[derive(Debug, Deserialize)]
pub struct StateConfig {
    pub action: String,
    pub next_state: Option<String>,
}

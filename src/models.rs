use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CallApiData {
    pub url: String,
    pub auth_header_name: String,
    pub auth_header_value: String,
}

#[derive(Debug, Default)]
pub struct ApiResponse {
    // Fields representing API response
}

#[derive(Debug, Default)]
pub struct ProcessedData {
    // Fields representing processed data
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct LlmData {
    pub user_prompt: String,
    pub system_prompt: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct AgentData {
    pub agent_config_file: String,
}

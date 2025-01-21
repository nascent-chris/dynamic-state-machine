use serde::{Deserialize, Serialize};

// Start of Selection
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
    PATCH,
    HEAD,
    OPTIONS,
}

impl From<&HttpMethod> for reqwest::Method {
    fn from(method: &HttpMethod) -> Self {
        match method {
            HttpMethod::GET => reqwest::Method::GET,
            HttpMethod::POST => reqwest::Method::POST,
            HttpMethod::PUT => reqwest::Method::PUT,
            HttpMethod::DELETE => reqwest::Method::DELETE,
            HttpMethod::PATCH => reqwest::Method::PATCH,
            HttpMethod::HEAD => reqwest::Method::HEAD,
            HttpMethod::OPTIONS => reqwest::Method::OPTIONS,
        }
    }
}

impl Default for HttpMethod {
    fn default() -> Self {
        HttpMethod::GET
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CallApiData {
    pub url: String,
    pub auth_header_name: String,
    pub auth_header_value: String,
    #[serde(default)]
    pub method: HttpMethod,
    pub body: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct LlmData {
    pub user_prompt: String,
    pub system_prompt: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct AgentData {
    pub agent_config_file: String,
    pub label: String,
    pub is_background: bool,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct WaitForInputData {
    pub data: String,
}

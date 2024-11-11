#[derive(Debug, thiserror::Error)]
pub enum LLMError {
    /// Network-related errors
    #[error("Network error: {0}")]
    Network(reqwest::Error),
    /// JSON parsing errors
    #[error("Failed to parse JSON: {0}")]
    Parse(reqwest::Error),
    /// Response parsing errors (missing fields, invalid format)
    #[error("Failed to parse response: {0}")]
    ResponseFormat(String),
    /// API-specific errors (rate limits, invalid auth, etc)
    #[error("API error: {0}")]
    ApiError(String),
    /// Tool execution errors
    #[error("Tool error: {0}")]
    ToolError(ToolError),
    /// Authentication-specific errors
    #[error("Authentication error: {0}")]
    Authentication(String),
    /// Stream-related errors
    #[error("Stream error: {0}")]
    StreamError(String),
    /// Forbidden access error
    #[error("Forbidden: {0}")]
    Forbidden(String),
    /// Not found error
    #[error("Not found: {0}")]
    NotFound(String),
    /// Server error
    #[error("Server error: {0}")]
    ServerError(String),
    /// I/O error
    #[error("I/O error: {0}")]
    IOError(String),
    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),
    /// Formatting error
    #[error("Formatting error: {0}")]
    FormatError(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    /// Tool not found error
    #[error("Tool not found: {0}")]
    ToolNotFound(String),
    /// Tool execution error
    #[error("Tool execution failed: {0}")]
    ExecutionError(String),
    /// Tool not enabled error
    #[error("Tool calls not enabled but llm tried to call a tool: {0}")]
    ToolCallsDisabled(String),
    /// Invalid argument error
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
}

impl From<ToolError> for LLMError {
    fn from(err: ToolError) -> Self {
        Self::ToolError(err)
    }
}

impl From<std::io::Error> for LLMError {
    fn from(err: std::io::Error) -> Self {
        Self::IOError(err.to_string())
    }
}

impl From<reqwest::Error> for LLMError {
    fn from(err: reqwest::Error) -> Self {
        // If the error has a status code, map it to a more specific error
        if let Some(status) = err.status() {
            match status.as_u16() {
                401 | 403 => Self::Authentication(format!("Authentication failed: {err}")),
                404 => Self::NotFound(format!("Resource not found: {err}")),
                429 => Self::ApiError(format!("Rate limit exceeded: {err}")),
                500..=599 => Self::ServerError(format!("Server error: {err}")),
                _ => Self::Network(err),
            }
        } else {
            // If no status code is available, default to Network error
            Self::Network(err)
        }
    }
}

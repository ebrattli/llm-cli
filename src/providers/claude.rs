use super::llm::LLMClient;
use crate::config::Config;
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::pin::Pin;

/// Base URL for the Anthropic Claude API
const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
/// API version header value
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Custom error type for Claude-specific errors
#[derive(Debug)]
pub enum ClaudeError {
    /// Errors returned by the Claude API
    ApiError(String),
    /// HTTP request errors
    RequestError(reqwest::Error),
    /// Header construction errors
    HeaderError(String),
    /// Stream processing errors
    StreamError(String),
}

impl fmt::Display for ClaudeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClaudeError::ApiError(msg) => write!(f, "Claude API error: {}", msg),
            ClaudeError::RequestError(e) => write!(f, "Request error: {}", e),
            ClaudeError::HeaderError(msg) => write!(f, "Header error: {}", msg),
            ClaudeError::StreamError(msg) => write!(f, "Stream error: {}", msg),
        }
    }
}

impl Error for ClaudeError {}

impl From<reqwest::Error> for ClaudeError {
    fn from(err: reqwest::Error) -> Self {
        ClaudeError::RequestError(err)
    }
}

/// Role types for Claude messages
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// User role for sending messages
    User,
    /// Assistant role for receiving responses
    Assistant,
}

/// Builder for constructing Claude API requests
#[derive(Debug, Serialize)]
pub struct ClaudeRequestBuilder {
    model: String,
    max_tokens: u32,
    messages: Vec<ClaudeMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

impl ClaudeRequestBuilder {
    /// Creates a new request builder with the specified model and token limit
    pub fn new(model: String, max_tokens: u32) -> Self {
        Self {
            model,
            max_tokens,
            messages: Vec::new(),
            stream: None,
        }
    }

    /// Adds a message to the request
    pub fn add_message(mut self, role: Role, content: String) -> Self {
        self.messages.push(ClaudeMessage { role, content });
        self
    }

    /// Enables or disables streaming for the request
    pub fn stream(mut self, enabled: bool) -> Self {
        self.stream = Some(enabled);
        self
    }

    /// Builds the final request
    pub fn build(self) -> ClaudeRequest {
        ClaudeRequest(self)
    }
}

/// Final request structure for the Claude API
#[derive(Debug, Serialize)]
pub struct ClaudeRequest(ClaudeRequestBuilder);

/// Message structure for Claude API requests
#[derive(Debug, Serialize)]
pub struct ClaudeMessage {
    #[serde(rename = "role")]
    role: Role,
    content: String,
}

/// Response structure for non-streaming requests
#[derive(Debug, Deserialize)]
pub struct ClaudeResponse {
    pub content: Vec<ClaudeContent>,
}

/// Event types for streaming responses
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamEventType {
    ContentBlockDelta,
    MessageStart,
    ContentBlockStart,
    ContentBlockStop,
    MessageDelta,
    MessageStop,
    Ping,
    Error,
}

/// Response structure for streaming events
#[derive(Debug, Deserialize)]
pub struct ClaudeStreamResponse {
    #[serde(rename = "type")]
    pub response_type: StreamEventType,
    pub delta: Option<StreamDelta>,
    pub message: Option<Message>,
}

/// Delta types for streaming responses
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum StreamDelta {
    Text {
        #[serde(rename = "type")]
        delta_type: String,
        text: String,
    },
    MessageDelta {
        stop_reason: Option<String>,
        stop_sequence: Option<String>,
    },
}

/// Message structure for streaming responses
#[derive(Debug, Deserialize)]
pub struct Message {
    pub id: String,
    #[serde(rename = "type")]
    pub message_type: String,
    pub role: String,
    pub content: Vec<Content>,
    pub model: String,
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
    pub usage: Usage,
}

/// Content structure for messages
#[derive(Debug, Deserialize)]
pub struct Content {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

/// Usage statistics for responses
#[derive(Debug, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// Content structure for non-streaming responses
#[derive(Debug, Deserialize)]
pub struct ClaudeContent {
    pub text: String,
}

/// Client for interacting with the Claude API
#[derive(Clone)]
pub struct ClaudeClient {
    api_key: String,
    client: Client,
}

impl ClaudeClient {
    /// Creates a new Claude client with the specified API key
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: Client::new(),
        }
    }

    /// Builds the required headers for API requests
    fn build_headers(&self) -> Result<HeaderMap, ClaudeError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-api-key",
            HeaderValue::from_str(&self.api_key).map_err(|e| ClaudeError::HeaderError(e.to_string()))?,
        );
        headers.insert(
            "anthropic-version",
            HeaderValue::from_static(ANTHROPIC_VERSION),
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        Ok(headers)
    }

    /// Creates a request with the specified message and configuration
    fn create_request(&self, message: &str, config: &Config, stream: bool) -> ClaudeRequest {
        let content = config
            .system_prompt
            .as_ref()
            .map_or(message.to_string(), |prompt| format!("{}\n\n{}", prompt, message));

        ClaudeRequestBuilder::new(config.claude.default_model.clone(), config.claude.max_tokens)
            .add_message(Role::User, content)
            .stream(stream)
            .build()
    }

    /// Handles API responses and deserializes them into the specified type
    async fn handle_response<T: for<'de> Deserialize<'de>>(
        &self,
        response: reqwest::Response,
    ) -> Result<T, ClaudeError> {
        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(ClaudeError::ApiError(error_text));
        }
        response.json::<T>().await.map_err(ClaudeError::from)
    }

    /// Processes a single stream event and extracts the text content
    fn process_stream_event(&self, line: &str) -> Result<String, ClaudeError> {
        if !line.starts_with("data: ") {
            return Ok(String::new());
        }

        let json_str = &line["data: ".len()..];
        if json_str == "[DONE]" {
            return Ok(String::new());
        }

        let response: ClaudeStreamResponse = serde_json::from_str(json_str)
            .map_err(|e| ClaudeError::StreamError(e.to_string()))?;

        match response.response_type {
            StreamEventType::ContentBlockDelta => {
                if let Some(StreamDelta::Text { text, .. }) = response.delta {
                    // Handle escaped newlines and normalize them
                    let text = text.replace("\\n", "\n");
                    // Trim any leading/trailing whitespace to prevent extra newlines
                    Ok(text.trim_matches('\n').to_string())
                } else {
                    Ok(String::new())
                }
            }
            _ => Ok(String::new()),
        }
    }
}

#[async_trait]
impl LLMClient for ClaudeClient {
    async fn send_message(
        &self,
        message: &str,
        config: &Config,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let request = self.create_request(message, config, false);
        let headers = self.build_headers()?;

        let response = self
            .client
            .post(ANTHROPIC_API_URL)
            .headers(headers)
            .json(&request)
            .send()
            .await?;

        let response_body: ClaudeResponse = self.handle_response(response).await?;
        let text = response_body
            .content
            .first()
            .map(|c| c.text.replace("\\n", "\n"))
            .unwrap_or_default();

        Ok(text)
    }

    async fn send_message_streaming(
        &self,
        message: &str,
        config: &Config,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, Box<dyn Error + Send + Sync>>> + Send>>, Box<dyn Error + Send + Sync>> {
        let request = self.create_request(message, config, true);
        let headers = self.build_headers()?;

        let response = self
            .client
            .post(ANTHROPIC_API_URL)
            .headers(headers)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(Box::new(ClaudeError::ApiError(error_text)));
        }

        let client = self.clone();
        let stream = response
            .bytes_stream()
            .map(move |result| {
                result
                    .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)
                    .and_then(|bytes| {
                        String::from_utf8(bytes.to_vec())
                            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)
                    })
                    .and_then(|text| {
                        let mut response_text = String::new();
                        for line in text.lines() {
                            let chunk = client
                                .process_stream_event(line)
                                .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
                            if !chunk.is_empty() {
                                if !response_text.is_empty() && !response_text.ends_with('\n') {
                                    response_text.push(' ');
                                }
                                response_text.push_str(&chunk);
                            }
                        }
                        Ok(response_text)
                    })
            })
            .filter(|result| futures::future::ready(result.as_ref().map(|s| !s.is_empty()).unwrap_or(true)));

        Ok(Box::pin(stream))
    }
}

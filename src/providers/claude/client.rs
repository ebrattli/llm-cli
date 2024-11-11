use crate::core::{Config, LLMError};
use crate::eventsource::{Event, EventSourceExt};
use crate::providers::llm::{BoxStream, LLMClient};
use crate::providers::Message as LLMMessage;
use crate::providers::MessageChunk as LLMMessageChunk;
use crate::tools::ToolDefinition as LLMToolDefinition;
use async_stream::try_stream;
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client, StatusCode,
};

use super::types::request::Tool;
use super::types::{
    ChatCompletionRequest, ContentBlock, DeltaEvent, Message, MessageResponse, StreamEvent,
};

const API_VERSION: &str = "2023-06-01";
const API_BASE_URL: &str = "https://api.anthropic.com/v1";

/// Client for interacting with the Claude API
pub struct ClaudeClient {
    api_key: String,
    client: Client,
    beta: Option<Vec<String>>,
    config: Config,
}

impl ClaudeClient {
    /// Create a new Claude client with the given API key
    pub fn new(api_key: String, config: Config) -> Self {
        Self {
            api_key,
            client: Client::new(),
            beta: None,
            config,
        }
    }

    /// Enable beta features for the client
    pub fn with_beta(mut self, beta_features: Vec<String>) -> Self {
        self.beta = Some(beta_features);
        self
    }

    /// Build headers for API requests
    fn build_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", HeaderValue::from_str(&self.api_key).unwrap());
        headers.insert("anthropic-version", HeaderValue::from_static(API_VERSION));

        if let Some(beta) = &self.beta {
            if let Ok(value) = HeaderValue::from_str(&beta.join(",")) {
                headers.insert("anthropic-beta", value);
            }
        }

        headers
    }

    async fn request_chat_completion(
        &self,
        request: ChatCompletionRequest<'_>,
        stream: bool,
    ) -> Result<reqwest::Response, LLMError> {
        let mut headers = self.build_headers();
        if stream {
            headers.insert("accept", HeaderValue::from_static("text/event-stream"));
        }

        let response = self
            .client
            .post(format!("{API_BASE_URL}/messages"))
            .headers(headers)
            .json(&request)
            .send()
            .await
            .map_err(|e| LLMError::ApiError(format!("Request failed: {e}")))?;

        match response.status() {
            StatusCode::OK => Ok(response),
            StatusCode::UNAUTHORIZED => Err(LLMError::ApiError(
                "Invalid API key or unauthorized access".to_string(),
            )),
            status => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(LLMError::ApiError(format!(
                    "API request failed with status {status}: {error_text}",
                )))
            }
        }
    }

    /// Convert event stream to text stream
    fn process_stream<'a, S>(stream: S) -> impl Stream<Item = Result<StreamEvent<'a>, LLMError>>
    where
        S: Stream<Item = Result<Event, reqwest::Error>> + Send + 'static,
    {
        stream.map(|event| {
            event
                .map_err(|e| LLMError::StreamError(e.to_string()))
                .and_then(StreamEvent::try_from)
        })
    }
}

impl TryFrom<Event> for StreamEvent<'_> {
    type Error = LLMError;
    fn try_from(event: Event) -> Result<Self, LLMError> {
        if event.data.is_empty() {
            Err(LLMError::StreamError("empty event data".to_string()))
        } else {
            serde_json::from_str(&event.data).map_err(|e| {
                LLMError::ApiError(format!(
                    "Failed to parse Claude stream event: {event}. Error: {e}"
                ))
            })
        }
    }
}

fn events_to_messages<'a, S>(mut stream: S) -> impl Stream<Item = Result<LLMMessageChunk, LLMError>>
where
    S: Stream<Item = Result<StreamEvent<'a>, LLMError>> + Send + Unpin,
{
    try_stream! {
        while let Some(event) = stream.next().await {
            let event = event?;
            match event {
                StreamEvent::MessageStart { .. } => continue,
                StreamEvent::ContentBlockStart { content_block, .. } => {
                    match content_block {
                        ContentBlock::ToolUse {id, name, ..} => yield LLMMessageChunk::ToolCallStart{ id: id.to_string(), name: name.to_string() },
                        _ => {}
                    }
                },
                StreamEvent::ContentBlockDelta { delta, .. } => {
                    match delta {
                        DeltaEvent::TextDelta { text } => yield LLMMessageChunk::Text(text),
                        DeltaEvent::InputJsonDelta { partial_json } => yield LLMMessageChunk::ToolCallArgument(partial_json)
                    }
                }
                StreamEvent::ContentBlockStop { .. } => yield LLMMessageChunk::ContentBlockStop,
                StreamEvent::MessageStop => yield LLMMessageChunk::stop(),
                StreamEvent::MessageDelta { .. } => continue,
                _ => continue,
            }
        }
    }
}

#[async_trait]
impl LLMClient for ClaudeClient {
    async fn query(
        &self,
        messages: &[LLMMessage],
        tools: Option<&[LLMToolDefinition]>,
    ) -> Result<Vec<LLMMessage>, LLMError> {
        let model = self.config.get_model();
        let max_tokens = self.config.get_max_tokens();

        let claude_messages: Vec<Message> = messages.iter().map(Message::from).collect();
        let mut request = ChatCompletionRequest::new(model, max_tokens, claude_messages);

        if let Some(tools) = tools {
            // TODO: Convert to Claude tool without cloning.
            let claude_tools: Vec<Tool> = tools.iter().map(Tool::from).collect();
            request = request.with_tools(claude_tools);
        }

        // Make API call
        let response = self.request_chat_completion(request, false).await?;
        let message_response: MessageResponse = response.json().await.map_err(|e| {
            LLMError::ResponseFormat(format!("Failed to parse Claude response: {e}"))
        })?;

        // Convert response to LLM message
        let content = message_response
            .content
            .into_iter()
            .fold(String::new(), |mut acc, block| {
                if let ContentBlock::Text { text } = block {
                    if !acc.is_empty() {
                        acc.push('\n');
                    }
                    acc.push_str(&text);
                }
                acc
            });

        Ok(vec![LLMMessage::Assistant {
            content,
            tool_calls: None,
        }])
    }

    async fn query_streaming(
        &self,
        messages: &[LLMMessage],
        tools: Option<&[LLMToolDefinition]>,
    ) -> Result<BoxStream, LLMError> {
        let model = self.config.get_model();
        let max_tokens = self.config.get_max_tokens();

        // Convert LLM messages to Claude messages
        let claude_messages: Vec<Message> = messages.iter().map(Message::from).collect();

        // Build request with streaming enabled
        let mut request =
            ChatCompletionRequest::new(model, max_tokens, claude_messages).with_stream(true);

        // Add tools if provided
        if let Some(tools) = tools {
            let claude_tools: Vec<Tool> = tools.iter().map(Tool::from).collect();
            request = request.with_tools(claude_tools);
        }

        // Make streaming API call
        let response = self.request_chat_completion(request, true).await?;

        let stream = Self::process_stream(response.events());
        Ok(events_to_messages(stream).boxed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Config, Provider, ProviderConfig};
    use crate::providers::Message as LLMMessage;
    use once_cell::sync::OnceCell;

    static CLIENT: OnceCell<ClaudeClient> = OnceCell::new();
    static CONFIG: OnceCell<Config> = OnceCell::new();

    fn get_test_config() -> &'static Config {
        CONFIG.get_or_init(|| Config {
            provider: Provider::Claude,
            system_prompt: None,
            claude: ProviderConfig {
                default_model: "claude-3-5-haiku-20241022".to_string(),
                max_tokens: 1024,
            },
            openai: ProviderConfig {
                default_model: "gpt-4o-mini".to_string(),
                max_tokens: 1024,
            },
            enable_tools: false,
            max_steps: 10,
            theme: None,
        })
    }

    fn get_client() -> &'static ClaudeClient {
        CLIENT.get_or_init(|| {
            dotenv::dotenv().expect("Failed to load .env file");
            let api_key = dotenv::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY not set");
            ClaudeClient::new(api_key, get_test_config().clone())
        })
    }

    #[tokio::test]
    async fn test_claude_send_message_invalid_key() {
        let config = get_test_config().clone();
        let client = ClaudeClient::new("invalid_key".to_string(), config);

        let messages = vec![LLMMessage::User {
            content: String::from("Hello, how are you?"),
        }];
        let response = client.query(&messages, None).await;

        match response {
            Err(LLMError::ApiError(ref error_msg)) => {
                println!("Detailed API Error: {error_msg}");
                assert!(
                    error_msg.to_lowercase().contains("invalid")
                        || error_msg.to_lowercase().contains("unauthorized"),
                    "Expected error message to indicate invalid key, got: {error_msg}"
                );
            }
            Err(other_error) => {
                panic!(
                    "Expected ApiError for invalid key, but got a different error type: {other_error:?}"
                );
            }
            Ok(_) => panic!("Unexpected successful response with invalid key"),
        }
    }

    #[tokio::test]
    async fn test_claude_send_message_streaming_invalid_key() {
        let config = get_test_config().clone();
        let client = ClaudeClient::new("invalid_key".to_string(), config);

        let messages = vec![LLMMessage::User {
            content: String::from("Hello, how are you?"),
        }];
        let stream_result = client.query_streaming(&messages, None).await;

        match stream_result {
            Err(LLMError::ApiError(ref error_msg)) => {
                println!("Detailed Streaming API Error: {error_msg}");
                assert!(
                    error_msg.to_lowercase().contains("invalid")
                        || error_msg.to_lowercase().contains("unauthorized"),
                    "Expected error message to indicate invalid key, got: {error_msg}"
                );
            }
            Err(other_error) => {
                panic!(
                    "Expected ApiError for invalid key in streaming, but got a different error type: {other_error:?}"
                );
            }
            Ok(_) => panic!("Unexpected successful streaming response with invalid key"),
        }
    }

    #[tokio::test]
    async fn test_claude_send_message() {
        let messages = vec![LLMMessage::User {
            content: String::from("Hello, how are you?"),
        }];
        let response = get_client().query(&messages, None).await;

        assert!(
            response.is_ok(),
            "Failed to send message. Error: {err:?}",
            err = response.as_ref().err()
        );

        let messages = response.expect("Response should be ok");
        let content = messages
            .first()
            .map(LLMMessage::content)
            .unwrap_or_default();
        assert!(
            !content.is_empty(),
            "Response content is empty. Received content: '{content}'"
        );
    }

    #[tokio::test]
    async fn test_claude_send_message_streaming() {
        let messages = vec![LLMMessage::User {
            content: String::from("Hello, how are you?"),
        }];
        let stream_result = get_client().query_streaming(&messages, None).await;

        assert!(
            stream_result.is_ok(),
            "Failed to create streaming response. Error: {err:?}",
            err = stream_result.as_ref().err()
        );

        let mut stream = stream_result.expect("Stream should be ok");
        let mut received_content = String::new();

        while let Some(chunk_result) = stream.next().await {
            assert!(
                chunk_result.is_ok(),
                "Streaming chunk failed. Error: {err:?}",
                err = chunk_result.err()
            );

            let chunk = chunk_result.expect("Chunk should be ok");
            if let LLMMessageChunk::Text(content) = chunk {
                if !content.is_empty() {
                    received_content.push_str(&content);
                }
            }
        }

        assert!(
            !received_content.is_empty(),
            "No content received during streaming. Received content: '{received_content}'"
        );
    }
}

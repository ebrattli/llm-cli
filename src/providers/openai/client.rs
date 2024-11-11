use crate::core::{Config, LLMError};
use crate::eventsource::{Event, EventSourceExt};
use crate::providers::llm::{BoxStream, LLMClient};
use crate::providers::openai::types::message::FinishReason;
use crate::providers::Message as LLMMessage;
use crate::providers::MessageChunk as LLMMessageChunk;
use crate::tools::ToolDefinition as LLMToolDefinition;
use async_stream::try_stream;
use futures::{Stream, StreamExt};
use reqwest::{Client, Response, StatusCode};

use super::types::{
    ChatCompletionChunk, ChatCompletionObject, ChatCompletionRequest, Message, Tool,
};

/// Constant for OpenAI Chat Completions API endpoint
const API_URL: &str = "https://api.openai.com/v1/chat/completions";

/// Client for interacting with OpenAI's API
///
/// Manages API key, configuration, and provides methods for sending messages
/// and streaming responses.
pub struct OpenAIClient {
    api_key: String,
    client: Client,
    config: Config,
}

impl OpenAIClient {
    /// Creates a new OpenAI client with the given API key and configuration
    ///
    /// # Arguments
    /// * `api_key` - Authentication token for OpenAI API
    /// * `config` - Configuration settings for the client
    pub fn new(api_key: String, config: Config) -> Self {
        Self {
            api_key,
            client: Client::new(),
            config,
        }
    }

    /// Creates a chat completion request to the OpenAI API
    ///
    /// # Arguments
    /// * `request` - The chat completion request configuration
    ///
    /// # Returns
    /// A `Result` with the API response or an error
    pub async fn create_chat_completion<'a>(
        &self,
        request: &'a ChatCompletionRequest<'a>,
    ) -> Result<Response, LLMError> {
        let response = self
            .client
            .post(API_URL)
            .header("Authorization", format!("Bearer {key}", key = self.api_key))
            .json(request)
            .send()
            .await
            .map_err(LLMError::from)?;

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
                    "API request failed with status {status}: {error_text}"
                )))
            }
        }
    }
}

#[async_trait::async_trait]
impl LLMClient for OpenAIClient {
    async fn query(
        &self,
        messages: &[LLMMessage],
        tools: Option<&[LLMToolDefinition]>,
    ) -> Result<Vec<LLMMessage>, LLMError> {
        // Implement the method to ensure the future is Send
        // Convert messages to OpenAI format upfront to ensure Send safety
        let openai_messages: Vec<Message> = messages.iter().map(Message::from).collect();

        let request = ChatCompletionRequest {
            model: self.config.get_model(),
            messages: openai_messages,
            temperature: Some(0.7),
            max_completion_tokens: Some(self.config.get_max_tokens()),
            tools: tools.map(|tools| tools.iter().map(Tool::from).collect()),
            ..Default::default()
        };

        let response = self.create_chat_completion(&request).await?;
        let response_text = response
            .text()
            .await
            .map_err(|e| LLMError::ResponseFormat(format!("Failed to get response text: {e}")))?;
        let chat_response: ChatCompletionObject =
            serde_json::from_str(&response_text).map_err(|e| {
                LLMError::ResponseFormat(format!("Failed to parse OpenAI response: {e}"))
            })?;

        let result = chat_response
            .choices
            .into_iter()
            .map(|choice| LLMMessage::from(choice.message))
            .collect();

        Ok(result)
    }

    async fn query_streaming(
        &self,
        messages: &[LLMMessage],
        tools: Option<&[LLMToolDefinition]>,
    ) -> Result<BoxStream, LLMError> {
        let openai_messages: Vec<Message> = messages.iter().map(Message::from).collect();

        let request = ChatCompletionRequest {
            model: self.config.get_model(),
            messages: openai_messages,
            temperature: Some(0.7),
            stream: true,
            max_completion_tokens: Some(self.config.get_max_tokens()),
            tools: tools.map(|tools| tools.iter().map(Tool::from).collect()),
            ..Default::default()
        };

        let response = self.create_chat_completion(&request).await?;
        let stream = process_stream(response.events());
        let message_stream = events_to_messages(stream);

        Ok(message_stream.boxed())
    }
}

fn events_to_messages(
    mut stream: impl Stream<Item = Result<ChatCompletionChunk, LLMError>> + Send + Unpin + 'static,
) -> impl Stream<Item = Result<LLMMessageChunk, LLMError>> + Send + 'static {
    try_stream! {
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            for choice in chunk.choices {
                if let Some(finish_reason) = choice.finish_reason {
                    match finish_reason {
                        FinishReason::Stop => yield LLMMessageChunk::stop(),
                        FinishReason::ToolCalls => yield LLMMessageChunk::ContentBlockStop,
                        FinishReason::Length => yield LLMMessageChunk::error(
                            "Response exceeded max tokens".to_string()
                        ),
                        FinishReason::ContentFilter => yield LLMMessageChunk::error(
                            "Content filter triggered".to_string()
                        ),
                    }
                } else {
                    let delta = choice.delta;
                    if let Some(content) = delta.content {
                        yield LLMMessageChunk::Text(content);
                    }
                    if let Some(tool_calls) = delta.tool_calls {
                        for tool_call in tool_calls {
                            if let (Some(id), Some(name)) = (tool_call.id, tool_call.function.name) {
                                yield LLMMessageChunk::ToolCallStart { id, name };
                            }
                            if !tool_call.function.arguments.is_empty() {
                                yield LLMMessageChunk::ToolCallArgument(tool_call.function.arguments);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn process_stream(
    mut stream: impl Stream<Item = Result<Event, reqwest::Error>> + Send + 'static + Unpin,
) -> impl Stream<Item = Result<ChatCompletionChunk, LLMError>> + Send + 'static + Unpin {
    let res = try_stream! {
        while let Some(event) = stream.next().await {
            match event {
                Ok(event) => {
                    if event.data == "[DONE]" {
                        continue;
                    }
                    yield ChatCompletionChunk::try_from(event)?;
                }
                Err(e) => Err(LLMError::StreamError(e.to_string()))?
            }
        }
    };
    Box::pin(res)
}

impl TryFrom<Event> for ChatCompletionChunk {
    type Error = LLMError;

    fn try_from(event: Event) -> Result<Self, LLMError> {
        if event.data.is_empty() {
            Err(LLMError::ResponseFormat("Empty data".to_string()))
        } else {
            serde_json::from_str(&event.data)
                .map_err(|e| LLMError::ResponseFormat(format!("Invalid JSON: {e}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Config, Provider, ProviderConfig};
    use once_cell::sync::OnceCell;

    static CLIENT: OnceCell<OpenAIClient> = OnceCell::new();
    static CONFIG: OnceCell<Config> = OnceCell::new();

    fn get_test_config() -> &'static Config {
        CONFIG.get_or_init(|| Config {
            provider: Provider::OpenAI,
            system_prompt: None,
            claude: ProviderConfig {
                default_model: String::from("claude-3-5-haiku-20241022"),
                max_tokens: 1024,
            },
            openai: ProviderConfig {
                default_model: String::from("gpt-4"),
                max_tokens: 1024,
            },
            enable_tools: false,
            max_steps: 10,
            theme: None,
        })
    }

    fn get_client() -> &'static OpenAIClient {
        CLIENT.get_or_init(|| {
            dotenv::dotenv().expect("Failed to load .env file");
            let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
            OpenAIClient::new(api_key, get_test_config().clone())
        })
    }

    #[tokio::test]
    async fn test_openai_send_message() {
        let messages = vec![LLMMessage::User {
            content: String::from("Hello, how are you?"),
        }];
        let response = get_client().query(&messages, None).await;

        assert!(
            response.is_ok(),
            "Failed to send message. Error: {:?}",
            response.as_ref().err()
        );

        let messages = response.expect("Response should be ok");
        let content = messages
            .first()
            .map(LLMMessage::content)
            .unwrap_or_default();
        assert!(
            !content.is_empty(),
            "Response content is empty. Received content: '{content}'",
        );
    }

    #[tokio::test]
    async fn test_openai_send_message_streaming() {
        let messages = vec![LLMMessage::User {
            content: String::from("Hello, how are you?"),
        }];
        let stream_result = get_client().query_streaming(&messages, None).await;

        assert!(
            stream_result.is_ok(),
            "Failed to create streaming response. Error: {:?}",
            stream_result.as_ref().err()
        );

        let mut stream = stream_result.expect("Stream should be ok");
        let mut received_content = String::new();

        while let Some(chunk_result) = stream.next().await {
            assert!(
                chunk_result.is_ok(),
                "Streaming chunk failed. Error: {:?}",
                chunk_result.err()
            );

            let chunk = chunk_result.expect("Chunk should be ok");
            if let LLMMessageChunk::Text(chunk_content) = chunk {
                if !chunk_content.is_empty() {
                    received_content.push_str(&chunk_content);
                }
            }
        }

        assert!(
            !received_content.is_empty(),
            "No content received during streaming. Received content: '{received_content}'",
        );
    }

    #[tokio::test]
    async fn test_openai_chat_completion() {
        let system_msg = String::from("You are a helpful assistant.");
        let user_msg = String::from("What's the weather like?");
        let request = ChatCompletionRequest {
            model: &get_test_config().openai.default_model,
            messages: vec![
                Message::System {
                    content: system_msg.into(),
                    name: None,
                },
                Message::User {
                    content: user_msg.into(),
                    name: None,
                },
            ],
            temperature: Some(0.7),
            max_completion_tokens: Some(get_test_config().openai.max_tokens),
            ..Default::default()
        };

        let response = get_client().create_chat_completion(&request).await;
        assert!(
            response.is_ok(),
            "Chat completion failed. Request model: {}. Error: {:?}",
            request.model,
            response.as_ref().err()
        );

        let completion_response = response.expect("Response should be ok");
        let response_text = completion_response
            .text()
            .await
            .expect("Response should have text");
        let completion: ChatCompletionObject =
            serde_json::from_str(&response_text).expect("Failed to parse chat completion response");

        assert!(
            !completion.choices.is_empty(),
            "No choices returned in chat completion. Request model: {}. Full response: {:?}",
            request.model,
            completion
        );

        let first_choice = &completion.choices[0];
        let content = match &first_choice.message {
            Message::Assistant { content, .. } => content,
            _ => "",
        };
        assert!(
                !content.is_empty(),
                "First choice message is empty or not from assistant. Request model: {}. Full response: {completion:?}",
                request.model
            );
    }

    #[tokio::test]
    async fn test_openai_send_message_invalid_key() {
        let config = get_test_config().clone();
        let client = OpenAIClient::new(String::from("invalid_key"), config);

        let messages = vec![LLMMessage::User {
            content: String::from("Hello, how are you?"),
        }];
        let response = client.query(&messages, None).await;

        assert!(response.is_err(), "Expected error with invalid API key");

        match response {
            Err(LLMError::ApiError(msg)) => {
                assert!(
                    msg.contains("Invalid API key"),
                    "Unexpected error message. Expected 'Invalid API key', got: '{msg}'"
                );
            }
            _ => panic!("Expected ApiError variant with invalid key"),
        }
    }

    #[tokio::test]
    async fn test_openai_send_message_streaming_invalid_key() {
        let config = get_test_config().clone();
        let client = OpenAIClient::new(String::from("invalid_key"), config);

        let messages = vec![LLMMessage::User {
            content: String::from("Hello, how are you?"),
        }];
        let stream_result = client.query_streaming(&messages, None).await;

        assert!(
            stream_result.is_err(),
            "Expected error with invalid API key"
        );

        match stream_result {
            Err(LLMError::ApiError(msg)) => {
                assert!(
                    msg.contains("Invalid API key"),
                    "Unexpected error message. Expected 'Invalid API key', got: '{msg}'"
                );
            }
            _ => panic!("Expected ApiError variant with invalid key"),
        }
    }

    #[tokio::test]
    async fn test_openai_conversation_history() {
        let system_msg = String::from("You are a helpful assistant.");
        let user_msg1 = String::from("My name is Alice.");
        let assistant_msg = String::from("Hello Alice! How can I help you today?");
        let user_msg2 = String::from("What's my name?");

        let request = ChatCompletionRequest {
            model: &get_test_config().openai.default_model,
            messages: vec![
                Message::System {
                    content: system_msg.into(),
                    name: None,
                },
                Message::User {
                    content: user_msg1.into(),
                    name: None,
                },
                Message::Assistant {
                    content: assistant_msg.into(),
                    name: None,
                    refusal: None,
                    tool_calls: None,
                },
                Message::User {
                    content: user_msg2.into(),
                    name: None,
                },
            ],
            temperature: Some(0.7),
            max_completion_tokens: Some(get_test_config().openai.max_tokens),
            ..Default::default()
        };

        let response = get_client().create_chat_completion(&request).await;
        assert!(
            response.is_ok(),
            "Chat completion failed. Request model: {}. Error: {:?}",
            request.model,
            response.as_ref().err()
        );

        let completion_response = response.expect("Response should be ok");
        let response_text = completion_response
            .text()
            .await
            .expect("Response should have text");
        let completion: ChatCompletionObject =
            serde_json::from_str(&response_text).expect("Failed to parse chat completion response");

        assert!(
            !completion.choices.is_empty(),
            "No choices returned in chat completion. Request model: {}. Full response: {:?}",
            request.model,
            completion
        );

        let response_content = match &completion.choices[0].message {
            Message::Assistant { content, .. } => content,
            _ => "",
        };

        assert!(
            response_content.contains("Alice"),
            "Response did not remember the name Alice. Full response content: '{response_content}'. Full response: {completion:?}",
        );
    }
}

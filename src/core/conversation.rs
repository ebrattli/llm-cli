use std::{io::Write, pin::Pin};

use crate::providers::types::messages::Message;
use crate::providers::{FinishReason, MessageChunk};
use crate::{
    core::{error::ToolError, formatter::Formatter, LLMError},
    tools::ToolCall,
};
use crate::{providers::llm::LLMClient, tools::ToolRegistry};
use futures::{Stream, StreamExt};
use log::debug;

use super::formatter::SyntaxHighlighter;

/// Manages the conversation loop between an LLM and its available tools.
/// Handles message streaming, tool execution, and conversation state.
pub struct ConversationManager {
    tool_registry: Option<ToolRegistry>,
    client: Box<dyn LLMClient>,
    formatter: Formatter<SyntaxHighlighter>,
}

impl ConversationManager {
    /// Creates a new `ConversationManager` with the specified LLM client and optional tool registry.
    ///
    /// # Arguments
    /// * `client` - The LLM client implementation to use for queries
    /// * `tool_registry` - Optional registry containing available tools
    /// * `formatter` - The formatter to use for output formatting
    pub fn new(
        client: Box<dyn LLMClient>,
        tool_registry: Option<ToolRegistry>,
        formatter: Formatter<SyntaxHighlighter>,
    ) -> Self {
        Self {
            tool_registry,
            client,
            formatter,
        }
    }

    /// Runs the conversation loop, processing messages and executing tools as needed.
    ///
    /// # Arguments
    /// * `initial_messages` - The starting messages for the conversation
    /// * `max_steps` - Maximum number of conversation turns to allow
    /// * `writer` - Output writer for streaming responses
    ///
    /// # Returns
    /// * `Result<Vec<Message>, LLMError>` - The final conversation messages or an error
    pub async fn run<W: Write + Send>(
        &mut self,
        initial_messages: Vec<Message>,
        max_steps: u32,
        writer: &mut W,
    ) -> Result<Vec<Message>, LLMError> {
        let mut conversation_state = ConversationState::new(initial_messages);
        let tool_definitions = self
            .tool_registry
            .as_ref()
            .map(ToolRegistry::get_tool_definitions);

        for i in 0..max_steps {
            debug!("[Conversation] step: {i}");
            let stream_response = self
                .client
                .query_streaming(&conversation_state.messages, tool_definitions.as_deref())
                .await?;

            let (content, tool_calls) = self.write_llm_response(stream_response, writer).await?;

            if tool_calls.is_empty() {
                conversation_state.add_assistant_message(content, tool_calls);
                debug!("[Conversation] No tool calls, ending conversation");
                break;
            }

            let tool_results = self.handle_tool_calls(&tool_calls).await?;
            debug!("[Conversation] Tool results: {:?}", tool_results);
            conversation_state.add_assistant_message(content, tool_calls);
            conversation_state.add_tool_results(tool_results);
        }

        Ok(conversation_state.messages)
    }

    /// Processes the LLM's streaming response, collecting content and tool calls.
    async fn write_llm_response<W: Write + Send>(
        &mut self,
        mut stream: Pin<Box<dyn Stream<Item = Result<MessageChunk, LLMError>> + Send>>,
        writer: &mut W,
    ) -> Result<(String, Vec<ToolCall>), LLMError> {
        let mut content = String::new();
        let mut tool_call_buffer = String::new();
        let mut tool_calls = Vec::new();
        let mut current_tool_call: Option<ToolCall> = None;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            match chunk {
                MessageChunk::Text(text) => {
                    self.write_chunk(writer, &text)?;
                    content.push_str(&text);
                }
                MessageChunk::ToolCallStart { id, name } => {
                    current_tool_call = Some(ToolCall {
                        id,
                        name,
                        arguments: serde_json::Value::Null,
                    });
                }
                MessageChunk::ToolCallArgument(tool_call_argument) => {
                    tool_call_buffer.push_str(&tool_call_argument);
                }
                MessageChunk::ContentBlockStop => {
                    if let Some(mut tool_call) = current_tool_call.take() {
                        tool_call.arguments = serde_json::from_str(&tool_call_buffer)
                            .unwrap_or(serde_json::Value::Null);
                        tool_calls.push(tool_call);
                        tool_call_buffer.clear();
                    }
                }
                MessageChunk::TextStart => continue,
                MessageChunk::End(finish_reason) => match finish_reason {
                    FinishReason::Stop => break,
                    FinishReason::Error(error) => {
                        return Err(LLMError::StreamError(error));
                    }
                },
            }
        }

        self.formatter.finish(writer)?;

        Ok((content, tool_calls))
    }

    /// Writes a chunk of content to the output writer.
    fn write_chunk<W: Write>(&mut self, writer: &mut W, content: &str) -> Result<(), LLMError> {
        self.formatter.format_chunk(writer, content)?;
        writer.flush()?;
        Ok(())
    }

    /// Executes a sequence of tool calls and returns their results.
    async fn handle_tool_calls(&self, tool_calls: &[ToolCall]) -> Result<Vec<Message>, ToolError> {
        let tool_registry = self.tool_registry.as_ref().ok_or_else(|| {
            let disabled_tools = tool_calls
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            ToolError::ToolCallsDisabled(disabled_tools)
        })?;

        let mut messages = Vec::with_capacity(tool_calls.len());

        for tool_call in tool_calls {
            let result = tool_registry
                .execute_tool(&tool_call.name, &tool_call.arguments)
                .await?;

            messages.push(Message::tool(result, &tool_call.id));
        }

        Ok(messages)
    }
}

/// Maintains the state of an ongoing conversation.
struct ConversationState {
    messages: Vec<Message>,
}

impl ConversationState {
    const fn new(initial_messages: Vec<Message>) -> Self {
        Self {
            messages: initial_messages,
        }
    }

    fn add_assistant_message(&mut self, content: String, tool_calls: Vec<ToolCall>) {
        self.messages.push(Message::assistant(
            content,
            (!tool_calls.is_empty()).then_some(tool_calls),
        ));
    }

    fn add_tool_results(&mut self, results: Vec<Message>) {
        self.messages.extend(results);
    }
}

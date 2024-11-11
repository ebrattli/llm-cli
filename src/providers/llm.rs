use crate::core::LLMError;
use crate::providers::Message;
use crate::tools::ToolDefinition;
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

use super::MessageChunk;

pub type BoxStream = Pin<Box<dyn Stream<Item = Result<MessageChunk, LLMError>> + Send + 'static>>;

#[async_trait]
pub trait LLMClient: Send + Sync {
    /// Query the LLM with a list of messages and optional tools
    async fn query(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
    ) -> Result<Vec<Message>, LLMError>;

    /// Query the LLM with streaming response and optional tools
    async fn query_streaming(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
    ) -> Result<BoxStream, LLMError>;
}

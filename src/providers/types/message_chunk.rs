/// Represents a chunk of a streaming message from a provider
/// This is a generic representation that both OpenAI and Claude chunks
/// can be converted into
#[derive(Debug, Clone)]
pub enum MessageChunk {
    /// A chunk containing text content
    Text(String),
    /// The start of a text chunk
    TextStart,
    /// Start of a tool call
    ToolCallStart { id: String, name: String },
    /// Content for a tool call's arguments (typically received in multiple chunks)
    ToolCallArgument(String),
    /// End of a tool call
    ContentBlockStop,
    /// Stream end marker with optional finish reason
    End(FinishReason),
}

#[derive(Debug, Clone)]
pub enum FinishReason {
    /// The model finished generating content
    Stop,
    /// The model encountered an error
    Error(String),
}

impl MessageChunk {
    /// Create a new tool call start chunk
    pub const fn tool_call_start(id: String, name: String) -> Self {
        Self::ToolCallStart { id, name }
    }

    /// Create a new end chunk with a finish reason
    pub const fn stop() -> Self {
        Self::End(FinishReason::Stop)
    }

    /// Create a new end chunk with an error reason
    pub const fn error(error: String) -> Self {
        Self::End(FinishReason::Error(error))
    }
}

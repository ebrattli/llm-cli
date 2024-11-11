use serde_json::Value;
use std::fmt;
use thiserror::Error;

use crate::tools::ToolCall;

#[derive(Debug)]
pub enum Message {
    User {
        content: String,
    },
    Assistant {
        content: String,
        tool_calls: Option<Vec<ToolCall>>,
    },
    ToolResult {
        content: Value,
        tool_call_id: String,
    },
}

impl Message {
    pub fn user(content: impl Into<String>) -> Self {
        Self::User {
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>, tool_calls: Option<Vec<ToolCall>>) -> Self {
        Self::Assistant {
            content: content.into(),
            tool_calls,
        }
    }

    pub fn tool(content: Value, tool_call_id: impl Into<String>) -> Self {
        Self::ToolResult {
            content,
            tool_call_id: tool_call_id.into(),
        }
    }

    pub fn content(&self) -> String {
        match self {
            Self::User { content } | Self::Assistant { content, .. } => content.to_string(),
            Self::ToolResult { content, .. } => content.to_string(),
        }
    }
}

#[derive(Debug, Error)]
pub enum MessageConversionError {
    #[error("Failed to serialize tool call arguments")]
    SerializationError(#[from] serde_json::Error),
    #[error("Invalid message format")]
    InvalidFormat,
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.content())
    }
}

use std::borrow::Cow;

use crate::providers::types::messages::Message as LLMMessage;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize, Debug)]
pub enum Role {
    #[serde(rename = "assistant")]
    Assistant,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum MessageContent<'a> {
    String(Cow<'a, str>),
    Array(Vec<ContentBlock<'a>>),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum ContentBlock<'a> {
    #[serde(rename = "text")]
    Text { text: Cow<'a, str> },
    #[serde(rename = "image")]
    Image { source: ImageSource },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: Cow<'a, str>,
        name: Cow<'a, str>,
        input: Cow<'a, serde_json::Value>,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: Cow<'a, str>,
        content: Cow<'a, Value>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImageSource {
    #[serde(rename = "type")]
    pub source_type: String, // Currently only "base64" is supported
    pub media_type: String, // "image/jpeg", "image/png", "image/gif", "image/webp"
    pub data: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "role")]
pub enum Message<'a> {
    #[serde(rename = "user")]
    User { content: MessageContent<'a> },
    #[serde(rename = "assistant")]
    Assistant { content: MessageContent<'a> },
}

impl<'a> Message<'a> {
    pub fn user(content: String) -> Self {
        Self::User {
            content: MessageContent::String(content.into()),
        }
    }

    pub const fn assistant(content: MessageContent<'a>) -> Self {
        Self::Assistant { content }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum MessageType {
    #[serde(rename = "message")]
    Message,
}

#[derive(Debug, Deserialize)]
pub struct MessageResponse<'a> {
    pub id: String,
    #[serde(rename = "type")]
    pub message_type: MessageType,
    pub role: Role,
    pub content: Vec<ContentBlock<'a>>,
    pub model: String,
    pub stop_reason: Option<StopReason>,
    pub stop_sequence: Option<String>,
    pub usage: Usage,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    StopSequence,
    ToolUse,
}

/// Token usage information for the request and response.
/// Note: During streaming, not all fields may be present in every event.
#[derive(Debug, Serialize, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: Option<i32>,
    #[serde(default)]
    pub output_tokens: Option<i32>,
    #[serde(default)]
    pub cache_read_input_tokens: Option<i32>,
    #[serde(default)]
    pub cache_creation_input_tokens: Option<i32>,
}

impl<'a> From<&'a LLMMessage> for Message<'a> {
    fn from(msg: &'a LLMMessage) -> Self {
        match msg {
            LLMMessage::User { content } => Self::user(content.into()),
            LLMMessage::ToolResult {
                content,
                tool_call_id,
            } => Self::User {
                content: MessageContent::Array(vec![ContentBlock::ToolResult {
                    tool_use_id: tool_call_id.into(),
                    content: Cow::Borrowed(content),
                }]),
            },
            LLMMessage::Assistant {
                content,
                tool_calls,
            } => {
                if tool_calls.is_none() {
                    Self::assistant(MessageContent::String(content.into()))
                } else {
                    let mut blocks = Vec::new();

                    if !content.is_empty() {
                        blocks.push(ContentBlock::Text {
                            text: content.into(),
                        });
                    }

                    if let Some(calls) = tool_calls {
                        for call in calls {
                            blocks.push(ContentBlock::ToolUse {
                                id: Cow::Borrowed(&call.id),
                                name: Cow::Borrowed(&call.name),
                                input: Cow::Borrowed(&call.arguments),
                            });
                        }
                    }

                    Self::assistant(MessageContent::Array(blocks))
                }
            }
        }
    }
}

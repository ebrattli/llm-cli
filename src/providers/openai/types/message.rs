use std::borrow::Cow;

use crate::providers::types::messages::Message as LLMMessage;
use crate::tools::ToolCall as LLMToolCall;
use crate::tools::ToolDefinition as LLMToolDefinition;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "role")]
#[serde(rename_all = "lowercase")]
pub enum Message<'a> {
    Developer {
        content: Cow<'a, str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    System {
        content: Cow<'a, str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    User {
        content: Cow<'a, str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    Assistant {
        content: Cow<'a, str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        refusal: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_calls: Option<Vec<ToolCall>>,
    },
    Tool {
        content: Cow<'a, Value>,
        tool_call_id: &'a str,
    },
}

impl<'a> Message<'a> {
    pub const fn developer(content: Cow<'a, str>) -> Self {
        Self::Developer {
            content,
            name: None,
        }
    }

    pub const fn system(content: Cow<'a, str>) -> Self {
        Self::System {
            content,
            name: None,
        }
    }

    pub const fn user(content: Cow<'a, str>) -> Self {
        Self::User {
            content,
            name: None,
        }
    }

    pub const fn assistant(content: Cow<'a, str>, tool_calls: Option<Vec<ToolCall>>) -> Self {
        Self::Assistant {
            content,
            name: None,
            refusal: None,
            tool_calls,
        }
    }

    pub const fn tool(content: Cow<'a, Value>, tool_call_id: &'a str) -> Self {
        Self::Tool {
            content,
            tool_call_id,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: CallType,
    pub function: FunctionCall,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CallType {
    #[serde(rename = "function")]
    Function,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Tool<'a> {
    #[serde(rename = "type")]
    pub tool_type: ToolType,
    #[serde(borrow)]
    pub function: Function<'a>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ToolType {
    #[serde(rename = "function")]
    Function,
}

impl<'a> From<&'a LLMToolDefinition> for Tool<'a> {
    fn from(tool_definition: &'a LLMToolDefinition) -> Self {
        Self {
            tool_type: ToolType::Function,
            function: Function {
                name: tool_definition.name.as_str(),
                description: Some(tool_definition.description.as_str()),
                parameters: Cow::Borrowed(&tool_definition.parameters),
                strict: None,
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Function<'a> {
    pub name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<&'a str>,
    pub parameters: Cow<'a, Value>,
    pub strict: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    None,
    Auto,
    Function { function: FunctionChoice },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FunctionChoice {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseFormat {
    #[serde(rename = "type")]
    pub format_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_schema: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StreamOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_size: Option<u32>,
}

impl<'a> From<Message<'a>> for LLMMessage {
    fn from(value: Message<'a>) -> Self {
        match value {
            Message::Assistant {
                content,
                tool_calls,
                ..
            } => Self::Assistant {
                content: content.into_owned(),
                tool_calls: tool_calls.map(|calls| {
                    calls
                        .into_iter()
                        .map(|call| LLMToolCall {
                            id: call.id,
                            name: call.function.name,
                            arguments: call.function.arguments,
                        })
                        .collect()
                }),
            },
            Message::Developer { content, .. }
            | Message::System { content, .. }
            | Message::User { content, .. } => Self::User {
                content: content.into_owned(),
            },
            Message::Tool {
                content,
                tool_call_id,
            } => Self::ToolResult {
                content: content.into_owned(),
                tool_call_id: tool_call_id.to_string(),
            },
        }
    }
}

impl<'a> From<&'a LLMMessage> for Message<'a> {
    fn from(msg: &'a LLMMessage) -> Self {
        match msg {
            LLMMessage::User { content } => Self::user(content.into()),
            LLMMessage::Assistant {
                content,
                tool_calls,
            } => {
                let tool_calls = tool_calls.as_ref().map(|calls| {
                    calls
                        .iter()
                        .map(|call| ToolCall {
                            id: call.id.to_string(),
                            call_type: CallType::Function,
                            function: FunctionCall {
                                name: call.name.to_string(),
                                arguments: Value::String(call.arguments.to_string()),
                            },
                        })
                        .collect()
                });
                Self::assistant(content.into(), tool_calls)
            }
            LLMMessage::ToolResult {
                content,
                tool_call_id,
            } => Self::tool(Cow::Borrowed(content), tool_call_id),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ContentFilter,
    ToolCalls,
}

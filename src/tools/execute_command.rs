use std::io::{self, Write};
use std::process::Command;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::core::error::ToolError;
use crate::tools::types::{Tool, ToolDefinition};

pub struct ExecuteCommandTool;

const CONFIRMATION_PROMPT: &str = "Do you want to execute the command: '{}' ? [y/N] ";
const COMMAND_EMPTY_ERROR: &str = "command cannot be empty";
const COMMAND_STRING_ERROR: &str = "command must be a string";

#[async_trait]
impl Tool for ExecuteCommandTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "execute_command".to_string(),
            description: "Executes a command on the command line and returns its output"
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The command to execute"
                    }
                },
                "required": ["command"]
            }),
        }
    }

    async fn execute(&self, arguments: &Value) -> Result<Value, ToolError> {
        let command = Self::extract_command(arguments)?;

        if !Self::confirm_execution(&command)? {
            return Ok(json!("stderr: Command execution cancelled by user"));
        }

        let (program, args) = Self::parse_command(&command)?;
        let output = Self::run_command(program, args)?;

        Ok(json!(Self::format_output(&output)))
    }
}

impl ExecuteCommandTool {
    /// Extracts command string from arguments
    fn extract_command(arguments: &Value) -> Result<String, ToolError> {
        arguments["command"]
            .as_str()
            .map(String::from)
            .ok_or_else(|| ToolError::InvalidArgument(String::from(COMMAND_STRING_ERROR)))
    }

    /// Prompts for user confirmation
    fn confirm_execution(command: &str) -> Result<bool, ToolError> {
        println!();
        print!("{}", CONFIRMATION_PROMPT.replace("{}", command));

        io::stdout()
            .flush()
            .map_err(|e| ToolError::ExecutionError(e.to_string()))?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|e| ToolError::ExecutionError(e.to_string()))?;

        Ok(matches!(input.trim().to_lowercase().as_str(), "y" | "yes"))
    }

    /// Parses command string into program and arguments
    fn parse_command(command: &str) -> Result<(&str, Vec<&str>), ToolError> {
        let mut parts = command.split_whitespace();
        let program = parts
            .next()
            .ok_or_else(|| ToolError::InvalidArgument(String::from(COMMAND_EMPTY_ERROR)))?;
        let args: Vec<&str> = parts.collect();

        Ok((program, args))
    }

    /// Executes the command and returns its output
    fn run_command(program: &str, args: Vec<&str>) -> Result<std::process::Output, ToolError> {
        Command::new(program)
            .args(args)
            .output()
            .map_err(|e| ToolError::ExecutionError(e.to_string()))
    }

    /// Formats command output into a presentable string
    fn format_output(output: &std::process::Output) -> String {
        let stdout = if output.stdout.is_empty() {
            String::new()
        } else {
            format!("stdout: {}", String::from_utf8_lossy(&output.stdout))
        };

        let stderr = if output.stderr.is_empty() {
            String::new()
        } else {
            format!(", stderr: {}", String::from_utf8_lossy(&output.stderr))
        };

        format!("{stdout}{stderr}")
    }
}

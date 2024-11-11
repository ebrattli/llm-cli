use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::{json, Value};
use thiserror::Error;

use crate::core::error::ToolError;
use crate::tools::types::{Tool, ToolDefinition};

/// The maximum number of commands that can be returned
const MAX_COMMAND_LIMIT: usize = 100;
/// The default number of commands to return if not specified
const DEFAULT_COMMAND_LIMIT: usize = 10;

/// Type alias for a Result with `HistoryError`
type HistoryResult<T> = Result<T, HistoryError>;

/// Errors that can occur when working with shell command history
#[derive(Debug, Error)]
pub enum HistoryError {
    /// The history file could not be found in the expected location
    #[error("History file not found")]
    NotFound,

    /// An error occurred while reading the history file
    #[error("Failed to read history: {0}")]
    ReadError(#[from] io::Error),

    /// The history file contains invalid or malformed data
    #[error("Invalid history format: {0}")]
    ParseError(String),
}

/// Trait for parsing shell-specific history formats
///
/// Implementations of this trait can parse different shell history file formats
/// and extract the actual commands from each line.
trait HistoryParser: Send + Sync {
    /// Attempts to parse a single line from a shell history file
    ///
    /// # Arguments
    ///
    /// * `line` - A line from the shell history file
    ///
    /// # Returns
    ///
    /// * `Some(String)` - The parsed command if successful
    /// * `None` - If the line is empty or invalid
    fn parse_line(&self, line: &str) -> Option<String>;
}

/// Parser for Zsh shell history format
///
/// Handles the extended history format used by Zsh:
/// `: timestamp:elapsed;command`
#[derive(Debug, Default)]
struct ZshParser;

impl HistoryParser for ZshParser {
    fn parse_line(&self, line: &str) -> Option<String> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        if trimmed.starts_with(": ") {
            trimmed.split_once(';').and_then(|(_, cmd)| {
                let cmd = cmd.trim();
                (!cmd.is_empty()).then_some(cmd.to_string())
            })
        } else {
            Some(trimmed.to_string())
        }
    }
}

/// Parser for Bash shell history format
///
/// Handles the simple line-based format used by Bash where each line
/// contains just the command.
#[derive(Debug, Default)]
struct BashParser;

impl HistoryParser for BashParser {
    fn parse_line(&self, line: &str) -> Option<String> {
        let trimmed = line.trim();
        (!trimmed.is_empty()).then_some(trimmed.to_string())
    }
}

/// Represents a shell history file with its associated parser
///
/// This struct handles reading and parsing shell history files from different
/// shell implementations (Zsh, Bash, etc.).
struct HistoryFile {
    path: PathBuf,
    parser: Box<dyn HistoryParser>,
}

impl HistoryFile {
    /// Creates a new `HistoryFile` instance
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the history file
    /// * `parser` - Parser implementation for the specific shell format
    fn new(path: PathBuf, parser: Box<dyn HistoryParser>) -> Self {
        Self { path, parser }
    }

    /// Attempts to detect and create a `HistoryFile` for the current user's shell
    ///
    /// Checks for common shell history files in the user's home directory
    /// and returns an appropriate `HistoryFile` instance.
    ///
    /// # Returns
    ///
    /// * `Ok(HistoryFile)` - If a supported history file is found
    /// * `Err(HistoryError)` - If no history file is found or there's an error
    fn detect() -> HistoryResult<Self> {
        let home = std::env::var("HOME").map_err(|_| HistoryError::NotFound)?;
        let home_path = PathBuf::from(home);

        // Try Zsh history first (more common on modern systems)
        let zsh_history = home_path.join(".zsh_history");
        if zsh_history.exists() {
            return Ok(Self::new(zsh_history, Box::new(ZshParser)));
        }

        // Fall back to Bash history
        let bash_history = home_path.join(".bash_history");
        if bash_history.exists() {
            return Ok(Self::new(bash_history, Box::new(BashParser)));
        }

        Err(HistoryError::NotFound)
    }

    /// Reads the most recent commands from the history file
    ///
    /// # Arguments
    ///
    /// * `limit` - Maximum number of commands to return
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<String>)` - List of recent commands
    /// * `Err(HistoryError)` - If there's an error reading or parsing the file
    fn read_recent_commands(&self, limit: usize) -> HistoryResult<Vec<String>> {
        let file = File::open(&self.path).map_err(HistoryError::ReadError)?;
        let reader = BufReader::new(file);

        let mut commands: Vec<String> = reader
            .lines()
            .filter_map(Result::ok)
            .filter_map(|line| self.parser.parse_line(&line))
            .collect();

        // Reverse to get most recent first and remove the current command
        commands.reverse();
        commands = commands.into_iter().skip(1).take(limit).collect();

        Ok(commands)
    }
}

/// Tool for retrieving recent shell command history
///
/// This tool provides access to the user's shell command history,
/// supporting both Zsh and Bash history formats.
#[derive(Debug, Default)]
pub struct CommandHistoryTool;

#[async_trait]
impl Tool for CommandHistoryTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "command_history".to_string(),
            description: "Retrieves the user's recently executed terminal commands.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "number",
                        "description": format!("Number of recent commands to retrieve from history (default: {}, max: {})", DEFAULT_COMMAND_LIMIT, MAX_COMMAND_LIMIT),
                        "minimum": 1,
                        "maximum": MAX_COMMAND_LIMIT
                    }
                }
            }),
        }
    }

    async fn execute(&self, arguments: &Value) -> Result<Value, ToolError> {
        #[allow(clippy::cast_possible_truncation)]
        let limit = arguments["limit"]
            .as_u64()
            .unwrap_or(DEFAULT_COMMAND_LIMIT as u64)
            .min(MAX_COMMAND_LIMIT as u64) as usize;

        let history_file = HistoryFile::detect().map_err(|e| {
            ToolError::ExecutionError(format!("Failed to locate history file: {e}"))
        })?;

        let commands = history_file
            .read_recent_commands(limit)
            .map_err(|e| ToolError::ExecutionError(format!("Failed to read history: {e}")))?;

        let result = format!("[{}]", commands.join(","));
        Ok(json!(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;
    use tokio;

    /// Helper function to create a temporary history file with given content
    fn create_temp_history(content: &[&str]) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        for line in content {
            writeln!(file, "{line}").unwrap();
        }
        file
    }

    mod parser_tests {
        use super::*;

        #[test]
        fn test_zsh_parser() {
            let parser = ZshParser;

            // Test cases
            let cases = [
                (": 1707394841:0;ls -la", Some("ls -la")),
                ("", None),
                (": 1707394841:0", None),
                (": 1707394841:0;  ", None),
                (": 1707394841:0;echo 'hello'", Some("echo 'hello'")),
                (": 1707394841:0;echo 'world'", Some("echo 'world'")),
            ];

            for (input, expected) in cases {
                assert_eq!(
                    parser.parse_line(input),
                    expected.map(String::from),
                    "Failed on input: {input}"
                );
            }
        }

        #[test]
        fn test_bash_parser() {
            let parser = BashParser;

            // Test cases
            let cases = [
                ("ls -la", Some("ls -la")),
                ("  cd /home  ", Some("cd /home")),
                ("", None),
                ("  ", None),
                ("echo 'hello'", Some("echo 'hello'")),
            ];

            for (input, expected) in cases {
                assert_eq!(
                    parser.parse_line(input),
                    expected.map(String::from),
                    "Failed on input: {input}"
                );
            }
        }
    }

    mod history_file_tests {
        use super::*;

        #[test]
        fn test_history_file_read_zsh() {
            let content = [
                ": 1707394841:0;ls -la",
                ": 1707394842:0;cd /home",
                ": 1707394843:0;echo 'hello'",
            ];
            let temp_file = create_temp_history(&content);
            let history = HistoryFile::new(temp_file.path().to_path_buf(), Box::new(ZshParser));

            let commands = history.read_recent_commands(2).unwrap();
            assert_eq!(commands, vec!["cd /home", "ls -la"]);
        }

        #[test]
        fn test_history_file_read_bash() {
            let content = ["ls -la", "cd /home", "echo 'hello'"];
            let temp_file = create_temp_history(&content);
            let history = HistoryFile::new(temp_file.path().to_path_buf(), Box::new(BashParser));

            let commands = history.read_recent_commands(2).unwrap();
            assert_eq!(commands, vec!["cd /home", "ls -la"]);
        }

        #[test]
        fn test_history_file_empty() {
            let temp_file = create_temp_history(&[]);
            let history = HistoryFile::new(temp_file.path().to_path_buf(), Box::new(BashParser));

            let commands = history.read_recent_commands(10).unwrap();
            assert!(commands.is_empty());
        }

        #[test]
        fn test_history_file_respects_limit() {
            let content: Vec<String> = (0..20).map(|i| format!("command {i}")).collect();
            let temp_file =
                create_temp_history(&content.iter().map(AsRef::as_ref).collect::<Vec<_>>());
            let history = HistoryFile::new(temp_file.path().to_path_buf(), Box::new(BashParser));

            let commands = history.read_recent_commands(5).unwrap();
            assert_eq!(commands.len(), 5);
            assert_eq!(commands[0], "command 18");
            assert_eq!(commands[4], "command 14");
        }
    }

    mod tool_tests {
        use super::*;

        #[tokio::test]
        async fn test_execute_with_limit() {
            let tool = CommandHistoryTool;
            let args = json!({ "limit": 5 });

            if let Ok(value) = tool.execute(&args).await {
                let num_commands = value.to_string().split(',').count();
                assert_eq!(num_commands, 5);
            }
        }

        #[tokio::test]
        async fn test_execute_default_limit() {
            let tool = CommandHistoryTool;
            let args = json!({});

            if let Ok(value) = tool.execute(&args).await {
                let num_commands = value.to_string().split(',').count();
                assert_eq!(num_commands, DEFAULT_COMMAND_LIMIT);
            }
        }

        #[tokio::test]
        async fn test_execute_max_limit() {
            let tool = CommandHistoryTool;
            let args = json!({ "limit": 200 }); // Exceeds maximum

            if let Ok(value) = tool.execute(&args).await {
                let num_commands = value.to_string().split(',').count();
                assert_eq!(num_commands, MAX_COMMAND_LIMIT);
            }
        }
    }
}

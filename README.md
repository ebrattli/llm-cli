# llm-cli

A powerful command-line interface for interacting with Large Language Models (LLMs) including OpenAI's GPT and Anthropic's Claude. Features an advanced tool execution system that allows the AI to run commands and interact with your system.

## Features

- Support for multiple LLM providers (OpenAI and Claude)
- Tool execution system allowing AI to run commands
- Configurable through TOML configuration
- Syntax highlighting for code responses
- Streaming responses for real-time interaction

## Installation

1. Ensure you have Rust and Cargo installed on your system
2. Clone this repository
3. Build in release mode:
```bash
cargo build --release
```
4. Install the binary to your system:
```bash
# Create the bin directory if it doesn't exist
mkdir -p ~/.local/bin
# Copy the binary
cp target/release/llm-cli ~/.local/bin/
```
5. Add `~/.local/bin` to your PATH if not already added:
```bash
# Add this to your ~/.bashrc or ~/.zshrc
export PATH="$HOME/.local/bin:$PATH"
```

## Configuration

Create or modify `config.toml` with your settings:

```toml
# Default provider and tool settings
provider = "claude"  # Options: "claude" or "openai"
enable_tools = true  # Enable/disable tool execution
max_steps = 10      # Maximum number of tool execution steps

[claude]
default_model = "claude-3-5-sonnet-20241022"
max_tokens = 8192

[openai]
default_model = "gpt-4"
max_tokens = 16383
```

You'll need to set your API keys as environment variables:
```bash
export OPENAI_API_KEY=your_openai_api_key
export ANTHROPIC_API_KEY=your_anthropic_api_key
```

## Usage

Basic usage:
```bash
llm-cli "Your question or prompt here"
```

### Options

- `--enable-tools`: Enable tool usage (AI can execute commands)
- `--max-steps <NUMBER>`: Maximum number of tool execution steps
- `-p, --provider <PROVIDER>`: Choose the LLM provider (openai or claude)
- `-d, --debug`: Enable debug output
- `-h, --help`: Display help information
- `-V, --version`: Display version information

### Examples

Basic query:
```bash
llm-cli "What is the capital of France?"
```

Using a specific provider:
```bash
llm-cli -p openai "Explain quantum computing"
```

Enable tool execution:
```bash
llm-cli --enable-tools "Create a new React project"
```

Limit tool execution steps:
```bash
llm-cli --enable-tools --max-steps 5 "Initialize a git repository"
```

## Dependencies

Key dependencies:
- clap: Command line argument parsing
- tokio: Async runtime
- reqwest: HTTP client
- serde: Serialization/deserialization
- syntect: Syntax highlighting
- config: Configuration management

## License

This project is open source and available under the MIT License.

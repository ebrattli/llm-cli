# llm-cli

A command-line interface for interacting with Large Language Models (LLMs) including OpenAI's GPT and Anthropic's Claude.

## Features

- Support for multiple LLM providers (OpenAI and Claude)
- Simple command-line interface
- Configurable model selection
- Environment-based API key configuration

## Installation

1. Ensure you have Rust and Cargo installed on your system
2. Clone this repository
3. Build the project:
```bash
cargo build --release
```

## Setup

Create a `.env` file in the project root with your API keys:

```env
OPENAI_API_KEY=your_openai_api_key
ANTHROPIC_API_KEY=your_anthropic_api_key
```

## Usage

Basic usage:
```bash
llm-cli "Your question or prompt here"
```

### Options

- `-p, --provider <PROVIDER>`: Choose the LLM provider (openai or claude) [default: openai]
- `-m, --model <MODEL>`: Optional model override
- `-h, --help`: Display help information
- `-V, --version`: Display version information

### Examples

Using OpenAI (default):
```bash
llm-cli "What is the capital of France?"
```

Using Claude:
```bash
llm-cli -p claude "Explain quantum computing"
```

Specifying a custom model:
```bash
llm-cli -p openai -m gpt-4 "Write a poem about rust programming"
```

## Dependencies

- clap: Command line argument parsing
- reqwest: HTTP client
- serde: Serialization/deserialization
- tokio: Async runtime
- dotenv: Environment variable management

## License

This project is open source and available under the MIT License.

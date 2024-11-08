use clap::Parser;
use dotenv::dotenv;
use futures::StreamExt;
use llm_cli::{Config, ClaudeClient, LLMClient, OpenAIClient, OutputFormatter};
use std::env;
use std::io::{self, Write};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Your query to the LLM
    #[arg()]
    query: Vec<String>,

    /// LLM provider to use (openai or claude)
    #[arg(short, long)]
    provider: Option<String>,

    /// Disable streaming mode
    #[arg(long)]
    no_stream: bool,
}

#[tokio::main]
async fn main() {
    // Force enable colors
    env::set_var("CLICOLOR", "1");
    env::set_var("CLICOLOR_FORCE", "1");
    env::set_var("TERM", "xterm-256color");
    env::remove_var("NO_COLOR");
    
    dotenv().ok();
    let args = Args::parse();
    let query = args.query.join(" ");

    // Load configuration
    let config = Config::load().expect("Failed to load config.toml");
    
    let client: Box<dyn LLMClient> = match args.provider.as_deref().unwrap_or(&config.default_provider) {
        "claude" => {
            let api_key = env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY not set in .env");
            Box::new(ClaudeClient::new(api_key))
        }
        "openai" => {
            let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set in .env");
            Box::new(OpenAIClient::new(api_key))
        }
        provider => panic!("Unsupported provider: {}", provider),
    };

    let formatter = OutputFormatter::new();
    let mut stdout = formatter.create_writer();
    // Removed the "Assistant: " prefix
    stdout.flush().unwrap();

    if args.no_stream {
        match client.send_message(&query, &config).await {
            Ok(response) => {
                formatter.write_output(&response, &mut stdout);
                stdout.flush().unwrap();
            }
            Err(e) => eprintln!("Error: {}", e),
        }
    } else {
        match client.send_message_streaming(&query, &config).await {
            Ok(mut stream) => {
                let mut buffer = String::new();
                let mut in_code_block = false;
                let mut code_block_buffer = String::new();

                while let Some(chunk) = stream.next().await {
                    match chunk {
                        Ok(text) => {
                            for c in text.chars() {
                                if buffer.ends_with("```") && !in_code_block {
                                    in_code_block = true;
                                    buffer.push(c);
                                } else if code_block_buffer.ends_with("```") && in_code_block {
                                    in_code_block = false;
                                    buffer.push_str(&code_block_buffer);
                                    buffer.push(c);
                                    code_block_buffer.clear();
                                } else if in_code_block {
                                    code_block_buffer.push(c);
                                } else {
                                    buffer.push(c);
                                }
                            }

                            // Try to find complete blocks to format
                            if let Some(idx) = find_format_boundary(&buffer) {
                                let to_format = buffer[..idx].to_string();
                                formatter.write_output(&to_format, &mut stdout);
                                stdout.flush().unwrap();
                                buffer = buffer[idx..].to_string();
                            }
                        }
                        Err(e) => {
                            eprintln!("\nError during streaming: {}", e);
                            break;
                        }
                    }
                }

                // Format any remaining content
                if !buffer.is_empty() {
                    formatter.write_output(&buffer, &mut stdout);
                    stdout.flush().unwrap();
                }
                println!(); // Print newline at end
            }
            Err(e) => eprintln!("Error: {}", e),
        }
    }
}

// Helper function to find appropriate boundaries for formatting in streaming mode
fn find_format_boundary(text: &str) -> Option<usize> {
    // Look for the end of a code block
    if let Some(idx) = text.find("```\n") {
        return Some(idx + 4);
    }
    
    // Look for sentence endings
    if let Some(idx) = text.rfind(|c| c == '.' || c == '!' || c == '?') {
        return Some(idx + 1);
    }
    
    // Look for newlines if no sentence endings found
    if let Some(idx) = text.rfind('\n') {
        return Some(idx + 1);
    }
    
    None
}

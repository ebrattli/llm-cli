use futures::StreamExt;
use llm_cli::config::Config;
use llm_cli::providers::claude::ClaudeClient;
use llm_cli::providers::llm::LLMClient;

fn get_test_config() -> Config {
    Config {
        default_provider: "claude".to_string(),
        system_prompt: None,
        claude: llm_cli::config::ProviderConfig {
            default_model: "claude-3-sonnet-20240229".to_string(),
            max_tokens: 1024,
        },
        openai: llm_cli::config::ProviderConfig {
            default_model: "gpt-4".to_string(),
            max_tokens: 1024,
        },
    }
}

#[tokio::test]
async fn test_claude_send_message() {
    let api_key = std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY not set");
    let client = ClaudeClient::new(api_key);
    let config = get_test_config();
    
    let response = client
        .send_message("Hello, how are you?", &config)
        .await;
        
    assert!(response.is_ok());
    let content = response.unwrap();
    assert!(!content.is_empty());
}

#[tokio::test]
async fn test_claude_send_message_streaming() {
    let api_key = std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY not set");
    let client = ClaudeClient::new(api_key);
    let config = get_test_config();
    
    let stream_result = client
        .send_message_streaming("Hello, how are you?", &config)
        .await;
        
    assert!(stream_result.is_ok());
    
    let mut stream = stream_result.unwrap();
    let mut received_content = false;
    
    while let Some(chunk_result) = stream.next().await {
        assert!(chunk_result.is_ok());
        let chunk = chunk_result.unwrap();
        if !chunk.is_empty() {
            received_content = true;
        }
    }
    
    assert!(received_content, "Should have received some content from the stream");
}

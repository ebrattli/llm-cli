use llm_cli::providers::openai::OpenAIClient;
use llm_cli::providers::llm::LLMClient;

#[tokio::test]
async fn test_openai_client() {
    let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
    let client = OpenAIClient::new(api_key);
    
    let response = client
        .send_message("Hello, how are you?", None, None)
        .await;
        
    assert!(response.is_ok());
    let content = response.unwrap();
    assert!(!content.is_empty());
}

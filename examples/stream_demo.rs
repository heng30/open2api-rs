use bot::{APIConfig, Chat, ChatConfig, StreamTextItem};
use std::io::Write;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    env_logger::init();

    let api_base_url =
        std::env::var("OPEN2API_URL").unwrap_or_else(|_| "http://localhost:8082/v1".to_string());
    let api_key = std::env::var("OPEN2API_API_KEY").unwrap_or_else(|_| "test-key-123".to_string());
    let model = std::env::var("OPEN2API_MODEL").unwrap_or_else(|_| "qwen3.5-plus".to_string());

    let request_config = APIConfig {
        api_base_url,
        api_model: model,
        api_key,
        temperature: Some(0.7),
        no_stream: Some(false),
        user_agent: None,
        request_timeout: 120,
    };

    let (tx, mut rx) = tokio::sync::mpsc::channel::<StreamTextItem>(100);
    let chat_config = ChatConfig { tx };

    let prompt = "You are a helpful assistant.";
    let question = "你好，请介绍一下 Rust 语言的特点。";
    let histories = vec![];

    let chat = Chat::new(prompt, question, chat_config, request_config, histories);

    let handle = tokio::spawn(async move {
        println!("=== 流式输出 ===");
        while let Some(item) = rx.recv().await {
            if item.finished {
                println!("\n=== 完成 ===");
                break;
            }
            if let Some(text) = item.text {
                print!("{}", text);
                std::io::stdout().flush().unwrap();
            }
            if let Some(err) = item.etext {
                println!("错误: {}", err);
            }
        }
    });

    if let Err(e) = chat.start().await {
        println!("聊天错误: {:?}", e);
    }

    _ = handle.await;
}

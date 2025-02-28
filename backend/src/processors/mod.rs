pub mod heurist_llm;
pub mod etherscan;
use async_openai::types::ChatCompletionRequestUserMessage;
use eyre::Result;
use tokio::sync::mpsc::Sender;
use std::path::PathBuf;
use crate::models::ForgeStep;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TemplatePattern {
    pub template: String,
    pub parameter_order: Vec<usize>,
    pub frequency: u64,
    pub success_rate: f64,
}

pub trait LLMGenerator {
    fn new(api_key: &str) -> Result<Self>
    where
        Self: Sized;
    async fn generate_forge_code(
        &mut self,
        address: &str,
        intent: &str,
        messages: &mut Vec<ChatCompletionRequestUserMessage>,
        tx: Sender<ForgeStep>,
    ) -> Result<String>;
    async fn fix_forge_code(
        &mut self,
        temp_dir: PathBuf,
        forge_error: &str,
        previous_messages: &mut Vec<ChatCompletionRequestUserMessage>,
        tx: Sender<ForgeStep>,
    ) -> Result<String>;
    async fn chat_stream(&self, messages: &[ChatCompletionRequestUserMessage], tx: Sender<ForgeStep>) -> Result<String>;

}

pub enum LLMImpl {
    Heurist(HeuristLLM),
}

impl LLMGenerator for LLMImpl {
    fn new(api_key: &str) -> Result<Self> {
        Ok(LLMImpl::Heurist(HeuristLLM::new(api_key)?))
    }

    async fn generate_forge_code(
        &mut self,
        address: &str,
        intent: &str,
        messages: &mut Vec<ChatCompletionRequestUserMessage>,
        tx: Sender<ForgeStep>,
    ) -> Result<String> {
        match self {
            LLMImpl::Heurist(llm) => llm.generate_forge_code(address, intent, messages, tx).await,
        }
    }
    
    async fn fix_forge_code(
        &mut self,
        temp_dir: PathBuf,
        forge_error: &str,
        previous_messages: &mut Vec<ChatCompletionRequestUserMessage>,
        tx: Sender<ForgeStep>,
    ) -> Result<String> {
        match self {
            LLMImpl::Heurist(llm) => {
                llm.fix_forge_code(temp_dir, forge_error, previous_messages, tx)
                    .await
            }
        }   
    }

    async fn chat_stream(&self, messages: &[ChatCompletionRequestUserMessage], tx: Sender<ForgeStep>) -> Result<String> {
        match self {
            LLMImpl::Heurist(llm) => llm.chat_stream(messages, tx   ).await,
        }
    }

}

pub use heurist_llm::LLMTemplateGenerator as HeuristLLM;

// pub fn extract_source_code(source_code: &str) -> Result<String> {
//     // Handle standard JSON format
//     if let Ok(json) = serde_json::from_str::<Value>(source_code) {
//         if let Some(sources) = json.get("sources") {
//             // Get the first source file that's not a library or interface
//             for (path, content) in sources.as_object().unwrap() {
//                 let file_content = content.get("content").and_then(|c| c.as_str());
//                 if let Some(content) = file_content {
//                     if !path.contains("/interfaces/")
//                         && !path.contains("/libraries/")
//                         && content.contains("contract")
//                     {
//                         return Ok(content.to_string());
//                     }
//                 }
//             }
//         }
//     }

//     // If not JSON, return as-is (single file format)
//     Ok(source_code.to_string())
// }

use async_openai::{
    config::OpenAIConfig,
    types::{ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs, ChatCompletionRequestUserMessage},
    Client as OpenAIClient,
};
use ethers::providers::StreamExt;
use eyre::{Result, eyre};
use std::fs;
use tokio::sync::mpsc::Sender;
use crate::models::ForgeStep;
use super::LLMGenerator;
use std::io::Write;
use std::path::PathBuf;

pub struct LLMTemplateGenerator {
    client: OpenAIClient<OpenAIConfig>,
}

impl LLMGenerator for LLMTemplateGenerator {
    fn new(api_key: &str) -> Result<Self> {
        Ok(Self {
            client: OpenAIClient::with_config(
                OpenAIConfig::new()
                    .with_api_key(api_key)
                    .with_api_base("https://llm-gateway.heurist.xyz")
            )
        })
    }

    async fn generate_forge_code(&mut self, address: &str, intent: &str, guidelines: &str, remappings: &str, messages: &mut Vec<ChatCompletionRequestUserMessage>, tx: Sender<ForgeStep>) -> Result<String> {
        let prompt = format!(
            "Generate a complete Solidity Forge script that implements the following user intent. \
            The script MUST STRICTLY use ONLY the following remappings for imports - do not deviate or make up paths:\n\
            ```\n{}\n```\n\
            Rules for imports:\n\
            1. ONLY use the exact paths from the remappings above\n\
            2. DO NOT create or assume any other import paths\n\
            3. If a required contract/interface is not in the remappings, you must include its full code\n\
            4. Each import must match exactly one of the remapping paths\n\n\
            Include all necessary imports, contract definitions, and a run() function. \
            The contract MUST inherit from forge-std/Script.sol and include 'import {{Script}} from \"forge-std/Script.sol\";'. \
            The script must not be a Test. \
            Never use the console from the std library. \
            The run() function must be marked as external and include vm.startBroadcast({}) and vm.stopBroadcast(). \
            Never use address(this), use the provided address {} instead. \
            Add comments explaining the key steps. \
            \nUser intent: {}\n\
            Guidelines: {}\n\
            Format the response as a complete Solidity file with SPDX license and pragma.", 
            remappings,
            address,
            address,
            intent,
            guidelines
        );

        println!("{}", remappings);

        messages.push(ChatCompletionRequestUserMessageArgs::default()
            .content(prompt)
            .build()?);

        self.chat_stream(messages, tx).await
    }

    async fn fix_forge_code(&mut self, temp_dir: PathBuf, forge_error: &str, messages: &mut Vec<ChatCompletionRequestUserMessage>, tx: Sender<ForgeStep>) -> Result<String> {
        // Get available libraries from lib folder
        let lib_path = temp_dir.join("lib");
        let available_libs = fs::read_dir(&lib_path)?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect::<Vec<_>>();

        // Read the original code
        let script_path = temp_dir.join("script").join("Script.s.sol");
        let original_code = fs::read_to_string(&script_path)?;

        // Read remappings
        let remappings = fs::read_to_string(temp_dir.join("remappings.txt"))?;

        let error_prompt = format!(
            "Fix the following Solidity Forge script that produced this error:\n\
            ERROR:\n{}\n\n\
            You MUST use ONLY these exact remappings for imports - do not deviate or make up paths:\n\
            ```\n{}\n```\n\
            Rules for fixing:\n\
            1. ONLY use the exact paths from the remappings above\n\
            2. DO NOT create or assume any other import paths\n\
            3. If a required contract/interface is not in the remappings, you must include its full code\n\
            4. Each import must match exactly one of the remapping paths\n\
            5. Available libraries in lib/: {}\n\n\
            Original code:\n\
            ```solidity\n{}\n```\n\n\
            Return the complete fixed script with SPDX license and pragma.\n\
            Ensure all imports are correct according to the remappings.", 
            forge_error,
            remappings,
            available_libs.join(", "),
            original_code
        );

        messages.push(ChatCompletionRequestUserMessageArgs::default()
            .content(error_prompt)
            .build()?);

        self.chat_stream(&messages, tx).await
    }

    async fn chat_stream(&self, messages: &[ChatCompletionRequestUserMessage], tx: Sender<ForgeStep>) -> Result<String> {
        let request = CreateChatCompletionRequestArgs::default()
            .model("qwen/qwen-2.5-coder-32b-instruct")
            .messages(messages.iter().map(|m| m.clone().into()).collect::<Vec<_>>())
            .max_tokens(2048u16)
            .temperature(0.3)
            .stream(true)
            .build()?;

        let mut stream = self.client.chat().create_stream(request).await?;
        let mut response = String::new();
        
        while let Some(result) = stream.next().await {
            match result {
                Ok(chat_response) => {
                    if let Some(ref content) = chat_response.choices[0].delta.content {
                        std::io::stdout().flush()?;
                        response.push_str(content);
                        tx.send(ForgeStep {
                            title: "Generating Code".to_string(),
                            output: content.clone(),
                        })
                        .await
                        .ok();
                    }
                }
                Err(e) => return Err(eyre!("Stream error: {}", e)),
            }
        }

        Ok(response)
    }

    async fn generate(&self, messages: &mut Vec<ChatCompletionRequestUserMessage>) -> Result<String> {
        let request = CreateChatCompletionRequestArgs::default()
            .model("mistralai/mixtral-8x7b-instruct")
            .messages(messages.iter().map(|m| m.clone().into()).collect::<Vec<_>>())
            .max_tokens(32u16)
            .temperature(0.1)
            .build()?;

        let mut stream = self.client.chat().create_stream(request).await?;
        let mut response = String::new();

        while let Some(result) = stream.next().await {
            match result {
                Ok(chat_response) => {
                    if let Some(ref content) = chat_response.choices[0].delta.content {
                        println!("{}", content);
                        response.push_str(content);
                    }
                }   
                Err(e) => return Err(eyre!("Stream error: {}", e)),
            }
        }

        Ok(response)
    }
}
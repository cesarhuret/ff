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

    async fn generate_forge_code(&mut self, address: &str, intent: &str, messages: &mut Vec<ChatCompletionRequestUserMessage>, tx: Sender<ForgeStep>) -> Result<String> {
        let prompt = format!(
            "Generate a complete Solidity Forge script that implements the following user intent. \
            Include all necessary imports, contract definitions, and a run() function. \
            The contract MUST inherit from forge-std/Script.sol and include 'import {{Script}} from \"forge-std/Script.sol\";'. \
            The script must not be a Test. \
            Never use the console from the std library. \
            The run() function must be marked as external and include vm.startBroadcast({}) and vm.stopBroadcast(). \
            Never use address(this), use the provided address {} instead. \
            Add comments explaining the key steps. \
            \nFor any token swaps:\
            1. ALWAYS get a quote for the expected output amount before executing the swap\
            2. Use the protocol's official quoting functions or contracts\
            3. Calculate slippage based on the quoted amount, not the input amount\
            4. Account for different token decimals in all calculations\
            5. Set reasonable deadlines with sufficient buffer time\
            6. Include clear comments showing the quote and slippage calculations\
            \nThe script should be ready to run with forge script. \
            If you are importing interfaces, you should either generate the interface code, or import them from github.\
            If you are importing libraries from github, they must be installed with forge install. Include the installation commands in your response after the script in escaped markdown code blocks in the format of '```sh' and '```'. \
            \nUser intent: {}\n\
            \nFormat the response as a complete Solidity file with SPDX license and pragma.", 
            address,
            address,
            intent
        );

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

        let error_prompt = format!(
            "The script failed with the following forge error:\n{}\n\
            Please fix the script to resolve these errors.\n\
            Available libraries in lib/ folder: {}\n\
            Import paths should use the following format:\n\
            - For OpenZeppelin: \"lib/openzeppelin-contracts/contracts/...\"\n\
            - For Uniswap: \"lib/v3-periphery/contracts/...\"\n\
            Return the complete fixed script.", 
            forge_error,
            available_libs.join(", ")
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
}
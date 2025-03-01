use eyre::{eyre, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use super::LLMGenerator;
use reqwest::Client;
use async_openai::types::ChatCompletionRequestUserMessageArgs;

pub struct ProtocolGuidelinesProcessor {
    guidelines_dir: PathBuf,
    guidelines: HashMap<String, String>,
}

impl ProtocolGuidelinesProcessor {
    pub fn new<P: AsRef<Path>>(guidelines_dir: P) -> Result<Self> {
        let dir_path = guidelines_dir.as_ref().to_path_buf();
        
        // Create directory if it doesn't exist
        if !dir_path.exists() {
            fs::create_dir_all(&dir_path)?;
        }
        
        let mut guidelines = HashMap::new();
        
        // Load existing guidelines
        if dir_path.exists() && dir_path.is_dir() {
            for entry in fs::read_dir(&dir_path)? {
                let entry = entry?;
                let path = entry.path();
                
                if path.is_file() && path.extension().map_or(false, |ext| ext == "md") {
                    if let Some(protocol_name) = path.file_stem().and_then(|s| s.to_str()) {
                        let content = fs::read_to_string(&path)?;
                        guidelines.insert(protocol_name.to_string(), content);
                    }
                }
            }
        }
        
        Ok(Self {
            guidelines_dir: dir_path,
            guidelines,
        })
    }
    
    pub async fn get_guideline(&self, llm: &impl LLMGenerator, intent: &str) -> Result<String> {
        let prompt = format!(
            "Based on this user input, determine which protocols the user is trying to interact with. \
            Return a concise list of the protocols in a json array. 
            Example output: \
            [\"uniswap_v3\"]
            The user input is: {}\n\
            The protocols are: {}",
            intent,
            self.guidelines.keys().cloned().collect::<Vec<_>>().join(", ")
        );

        let mut messages = Vec::new();
        messages.push(ChatCompletionRequestUserMessageArgs::default()
        .content(prompt)
        .build()?);

        let content = llm.generate( &mut messages).await?;

        // Extract just the JSON array part using a more robust approach
        let protocols = content
            .lines()  // Split into lines
            .find(|line| line.trim().starts_with('['))  // Find the line that starts with [
            .ok_or_else(|| eyre::eyre!("No JSON array found in response"))?;  // Error if not found

        let protocols: Vec<String> = serde_json::from_str(protocols)?;

        let mut guidelines = String::new();
        for protocol in protocols {
            guidelines.push_str(&self.guidelines[&protocol]);
            guidelines.push_str("\n\n");
        }

        Ok(guidelines)
    }
    
    pub fn available_protocols(&self) -> Vec<String> {
        self.guidelines.keys().cloned().collect()
    }
    
    pub async fn generate_guidelines(
        &self,
        llm: &impl LLMGenerator,
        protocol: String,
        doc_links: Vec<String>,
    ) -> Result<String> {

        // fetch the doc links
        let doc_links = fetch_doc_links(doc_links).await?;
        
        let prompt = format!(
            "Generate comprehensive guidelines for the {} protocol that will help an AI assistant generate secure, production-ready, bug-free Solidity code that can be executed. \
            The guidelines should include:\n\
            1. Protocol overview\n\
            2. Core functions and example implementation / template \n\
            3. Security considerations specific to the protocol\n\
            4. Common pitfalls\n\
            6. Deployment Addresses\n\
            Use the following documentation as reference:\n{}\n\n\
            Format the output as a markdown document with appropriate sections, code examples, and security warnings. \
            The code examples should be production-ready and follow all security best practices.
            For example, if anything needs to be done on the frontend, like reading some onchain data, you should include what function is called to read the data.
            An example of this is when you get a quote from the Uniswap V3 Quoter contract, you should include the function call to get the quote, and use that result for slippage.
            The documentation will sometimes have examples, you should use them and the core docs as reference in 2. Core functions and example implementation / template.
            Also make sure to include all the functions that are provided, and don't make up your own functions or use made up functions from the docs - they should be calling the contract functions.
            Always include ALL the deployment addresses for the protocol, but you can ignore testnet addresses.",
            protocol,
            doc_links.join("\n")
        );

        println!("Generating guidelines for {}", protocol);

        let mut messages = Vec::new();
        messages.push(ChatCompletionRequestUserMessageArgs::default()
        .content(prompt)
        .build()?);


        let content = llm.generate( &mut messages).await?;
        
        // Save to file
        let file_path = self.guidelines_dir.join(format!("{}.md", protocol));
        fs::write(&file_path, &content)?;
        
        Ok(content)
    }
}

async fn fetch_doc_links(links: Vec<String>) -> Result<Vec<String>> {
    let client = Client::new();
    let mut contents = Vec::new();

    for link in links {
        // Convert github.com URLs to raw.githubusercontent.com if needed
        let raw_url = if link.contains("github.com") {
            link.replace("github.com", "raw.githubusercontent.com")
                .replace("/blob/", "/")
        } else {
            link
        };

        let response = client.get(&raw_url)
            .send()
            .await
            .map_err(|e| eyre!("Failed to fetch documentation from {}: {}", raw_url, e))?;

        if !response.status().is_success() {
            return Err(eyre!("Failed to fetch {}: HTTP {}", raw_url, response.status()));
        }

        let content = response.text()
            .await
            .map_err(|e| eyre!("Failed to read response from {}: {}", raw_url, e))?;

        contents.push(content);
    }

    Ok(contents)
} 
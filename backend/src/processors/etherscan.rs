use serde::Deserialize;
use reqwest::Client;
use eyre::Result;


#[derive(Debug, Deserialize)]
pub struct ContractInfo {
    #[serde(rename = "SourceCode")]
    pub source_code: String,
    #[serde(rename = "ContractName")]
    pub contract_name: String,
    #[serde(rename = "ABI")]
    pub abi: String,
}

#[derive(Debug, Deserialize)]
pub struct EtherscanResponse<T> {
    status: String,
    message: String,
    result: T,
}

pub async fn get_etherscan_contract(address: &str, api_key: &str) -> Result<ContractInfo> {
    let client = Client::new();
    let url = format!(
        "https://api.etherscan.io/api?module=contract&action=getsourcecode&address={}&apikey={}",
        address, api_key
    );

    let response = client.get(&url).send().await?;
    let data: EtherscanResponse<Vec<ContractInfo>> = response.json().await?;

    data.result
        .into_iter()
        .next()
        .ok_or_else(|| eyre::eyre!("No contract found"))
}

pub fn extract_contract_source(contract_info: &ContractInfo) -> Result<String> {
    let source_code = &contract_info.source_code;

    // Remove the leading/trailing {{ and }} if present
    let source_code = source_code.trim_start_matches("{{").trim_end_matches("}}");

    // Clean up any whitespace/newlines at start/end
    let source_code = source_code.trim();

    // Add opening and closing braces to make it valid JSON
    let source_code = format!("{{{}}}", source_code);

    // First decode: handle the escaped JSON string
    let decoded = match serde_json::from_str::<serde_json::Value>(&source_code) {
        Ok(d) => d,
        Err(e) => {
            println!("JSON parse error: {}", e);
            println!("First few characters: {:?}", &source_code[..50]);
            return Err(eyre::eyre!("Failed to parse JSON: {}", e));
        }
    };

    // Get the sources object
    let sources = decoded
        .get("sources")
        .ok_or_else(|| eyre::eyre!("No sources found"))?
        .as_object()
        .ok_or_else(|| eyre::eyre!("Sources is not an object"))?;

    // Find the contract file
    for (path, content) in sources {
        if path.ends_with(&format!("{}.sol", contract_info.contract_name)) {
            // Get the content string which is also escaped
            let content_str = content
                .get("content")
                .and_then(|c| c.as_str())
                .ok_or_else(|| eyre::eyre!("No content found"))?;

            // Second decode: unescape the actual source code
            let unescaped = content_str
                .replace("\\r\\n", "\n")
                .replace("\\\"", "\"")
                .replace("\\\\", "\\");

            return Ok(unescaped);
        }
    }

    Err(eyre::eyre!("Contract source not found"))
}

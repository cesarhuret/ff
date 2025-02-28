use eyre::Result;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct AlchemyResponse {
    jsonrpc: String,
    id: i64,
    result: TokenBalancesResult,
}

#[derive(Debug, Deserialize)]
pub struct TokenBalancesResult {
    address: String,
    tokenBalances: Vec<TokenBalance>,
    pageKey: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenBalance {
    contractAddress: String,
    tokenBalance: String,
}

pub async fn get_token_balances(address: &str, api_key: &str) -> Result<TokenBalancesResult> {
    let client = reqwest::Client::new();
    let url = format!("https://eth-mainnet.g.alchemy.com/v2/{}", api_key);

    let response = client
        .post(&url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "alchemy_getTokenBalances",
            "params": [address, "erc20"],
            "id": 42
        }))
        .send()
        .await?
        .json::<AlchemyResponse>()
        .await?;

    Ok(response.result)
}
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use serde_json::Value;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::Mutex;
use tokio::sync::Semaphore;
use crate::processors::LLMImpl;
use async_openai::types::ChatCompletionRequestUserMessage;
use crate::ProtocolGuidelinesProcessor;
use std::path::PathBuf;

#[derive(Serialize, Debug)]
pub struct ForgeStep {
    pub title: String,
    pub output: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ForgeTransaction {
    pub hash: Option<String>,
    pub transactionType: String,
    pub contractName: Option<String>,
    pub contractAddress: String,
    pub function: String,
    pub arguments: Vec<String>,
    pub transaction: ForgeTransactionDetails,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ForgeTransactionDetails {
    pub from: String,
    pub to: String,
    pub gas: String,
    pub value: String,
    pub input: String,
    pub nonce: String,
    pub chainId: String,
}

#[derive(Debug, Deserialize)]
pub struct ForgeOutput {
    pub transactions: Vec<ForgeTransaction>,
    pub receipts: Vec<Value>,
    pub libraries: Vec<Value>,
    pub pending: Vec<Value>,
    pub returns: HashMap<String, Value>,
    pub timestamp: u64,
    pub chain: u64,
    pub commit: Option<String>,
}

#[derive(Deserialize)]
pub struct ForgeRequest {
    pub intent: String,
    pub from_address: String,
    pub rpc_url: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct ForgeResponse {
    pub transactions: Vec<ForgeTransaction>,
}

#[derive(Debug, Serialize)]
pub struct Transaction {
    pub to: String,
    pub data: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct TransactionDetails {
    pub to: String,
    pub function: String,
    pub arguments: Vec<String>,
    pub value: String,
    pub input_data: String,
} 


pub struct AppState {
    pub template_generator: Mutex<LLMImpl>,
    pub process_limiter: Arc<Semaphore>,
    pub temp_dirs: Mutex<HashMap<String, TempDir>>,
    pub protocol_processor: Arc<ProtocolGuidelinesProcessor>,
    pub base_forge_dir: PathBuf,
}

#[derive(Deserialize)]
pub struct FixRequest {
    pub error: String,
    pub temp_dir: String,
    pub rpc_url: Option<String>,
}


#[derive(Serialize, Deserialize, Debug)]
pub struct SessionData {
    pub messages: Vec<ChatCompletionRequestUserMessage>,
}

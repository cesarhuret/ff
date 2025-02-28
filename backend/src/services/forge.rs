use crate::models::Transaction;
use eyre::Result;

pub fn parse_forge_output(output: &str) -> Result<Vec<Transaction>> {
    let mut transactions = Vec::new();

    // Example parsing - adjust based on actual forge output format
    for line in output.lines() {
        if line.contains("Contract call:") || line.contains("Transaction:") {
            // Parse transaction details from the line
            // This is a simplified example - you'll need to adjust the parsing
            // based on the actual forge output format
            if let (Some(to), Some(data)) = (
                line.split("to:").nth(1).map(str::trim),
                line.split("data:").nth(1).map(str::trim),
            ) {
                transactions.push(Transaction {
                    to: to.to_string(),
                    data: data.to_string(),
                    value: "0".to_string(), // Parse actual value if present
                });
            }
        }
    }

    Ok(transactions)
}

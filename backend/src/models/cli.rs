use std::path::PathBuf;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the web server
    Server,
    
    /// Generate protocol guidelines
    GenerateGuidelines {
        /// Protocol name (e.g., uniswap_v2, aave_v3)
        #[arg(short, long)]
        protocol: String,
        
        /// Documentation links, comma-separated
        #[arg(short, long)]
        links: String,
        
        /// Output directory for markdown files
        #[arg(short, long, default_value = "./guidelines")]
        output_dir: PathBuf,
        

    },
}

#[derive(Parser, Debug)]
pub struct GenerateArgs {
    #[arg(short, long)]
    pub address: String,

    #[arg(short, long)]
    pub key: String,

    #[arg(long)]
    pub local: bool,
}

#[derive(Parser, Debug)]
pub struct ForgeArgs {
    #[arg(short, long)]
    pub intent: String,
    
    #[arg(short, long)]
    pub private_key: String,
    
    #[arg(short, long)]
    pub from: String,
    
    #[arg(short, long)]
    pub rpc_url: Option<String>,
} 
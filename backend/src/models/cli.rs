use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Parser, Debug)]
pub enum Commands {
    Forge(ForgeArgs),
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
mod processors;
mod models;
mod handlers;
mod utils;

use crate::processors::{
    HeuristLLM, LLMGenerator, LLMImpl, ProtocolGuidelinesProcessor,
};
use axum::{
    routing::get,
    Router,
    extract::State,
};
use eyre::Result;
use handlers::{stream_forge_process, fix_forge_process};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::Semaphore;
use tower_http::{
    cors::{CorsLayer, Any},
    trace::{self, TraceLayer},
};
use tracing::{info, Level};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use crate::models::{Cli, Commands, AppState};
use std::path::PathBuf;
use clap::Parser;
use eyre::eyre;
use std::fs;
use std::process::Command;
use crate::utils::run_command_with_output;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "backend=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Server) => {
            run_server().await?;
        },
        Some(Commands::GenerateGuidelines { protocol, links, output_dir  }) => {
            generate_protocol_guidelines(protocol, links, output_dir).await?;
        },
        None => {
            // Default to running the server if no command is provided
            run_server().await?;
        }
    }

    Ok(())
}

async fn run_server() -> Result<()> {
    info!("Starting server...");

    let base_forge_dir = initialize_base_project().await?;
    
    // Initialize protocol guidelines
    let protocol_processor = ProtocolGuidelinesProcessor::new("./guidelines")?;
    info!("Loaded protocol guidelines: {:?}", protocol_processor.available_protocols());

    let template_generator = LLMImpl::Heurist(HeuristLLM::new("cesar#huret-1")?);
    let state = Arc::new(AppState {
        template_generator: Mutex::new(template_generator),
        process_limiter: Arc::new(Semaphore::new(100)),
        temp_dirs: Mutex::new(HashMap::new()),
        protocol_processor: Arc::new(protocol_processor),
        base_forge_dir,
    });

    let app = Router::new()
        .route("/forge/stream", get(stream_forge_process))
        .route("/forge/fix", get(fix_forge_process))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(trace::DefaultMakeSpan::new()
                    .level(Level::INFO))
                .on_response(trace::DefaultOnResponse::new()
                    .level(Level::INFO)),
        )
        .layer(CorsLayer::permissive())
        .with_state(state);

    info!("Routes registered: {:?}", app);

    let addr = "0.0.0.0:3000";
    info!("Listening on http://{}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn generate_protocol_guidelines(
    protocol: String, 
    links: String, 
    output_dir: PathBuf,
) -> Result<()> {
    info!("Generating guidelines for protocol: {}", protocol);
    
    let protocol_processor = ProtocolGuidelinesProcessor::new(&output_dir)?;
    let llm = HeuristLLM::new("cesar#huret-1")?;
    
    // Parse comma-separated links
    let doc_links: Vec<String> = links.split(',')
        .map(|s| s.trim().to_string())
        .collect();
    
    info!("Using documentation links: {:?}", doc_links);
    
    // Generate guidelines using processor
    let content = protocol_processor.generate_guidelines(&llm, protocol.clone(), doc_links).await?;
    info!("Guidelines generated successfully");

    // Save to file
    let file_path = output_dir.join(format!("{}.md", protocol));
    fs::write(&file_path, &content)?;
    
    Ok(())
}

async fn initialize_base_project() -> Result<PathBuf> {
    info!("Initializing base forge project...");
    
    let base_dir = PathBuf::from("./base_forge_project");
    if !base_dir.exists() {
        fs::create_dir_all(&base_dir)?;
        
        // Initialize forge project
        Command::new("forge")
            .args(&["init", "--no-commit"])
            .current_dir(&base_dir)
            .output()?;

        // Install dependencies
        let dependencies = [
            "openzeppelin/openzeppelin-contracts",
            "Uniswap/v3-core",
            "Uniswap/v3-periphery",
            "aave/aave-v3-core",
            "aave/aave-v3-periphery"
        ];

        for dep in dependencies.iter() {
            Command::new("forge")
                .args(&["install", dep, "--no-commit"])
                .current_dir(&base_dir)
                .output()?;
        }

        // Generate remappings
        let output = Command::new("forge")
            .args(&["remappings"])
            .current_dir(&base_dir)
            .output()?;

        // Write the output to remappings.txt
        fs::write(
            base_dir.join("remappings.txt"),
            output.stdout
        )?;
    }

    Ok(base_dir)
}
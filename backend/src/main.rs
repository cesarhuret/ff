mod processors;
mod models;
mod handlers;
mod services;
mod utils;

use crate::processors::{
    HeuristLLM, LLMGenerator, LLMImpl,
};
use axum::{ routing::get, Router};
use eyre::Result;
use handlers::{stream_forge_process, fix_forge_process};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::Semaphore;
use tower_http::{
    cors::CorsLayer,
    trace::{self, TraceLayer},
};
use tracing::{info, Level};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use crate::models::AppState;


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

    info!("Starting server...");

    let template_generator = LLMImpl::Heurist(HeuristLLM::new("cesar#huret-1")?);
    let state = Arc::new(AppState {
        template_generator: Mutex::new(template_generator),
        process_limiter: Arc::new(Semaphore::new(100)),
        temp_dirs: Mutex::new(HashMap::new()),
    });

    // Build our application with a route
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

    let addr = "127.0.0.1:3000";
    info!("Listening on http://{}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}


use crate::models::{ForgeOutput, ForgeRequest, ForgeStep, AppState, FixRequest, SessionData, TransactionDetails};
use crate::utils::{run_command_with_output, install_dependencies};
use axum::{
    extract::{Query, State},
    response::sse::{Event, Sse},
};
use eyre::Result;
use futures::stream::{self, Stream};
use std::{convert::Infallible, fs, sync::Arc};
use tokio::process::Command;
use uuid::Uuid;
use tempfile::TempDir;
use std::path::PathBuf;
use crate::processors::LLMGenerator;
use fs_extra::dir::copy;


pub async fn fix_forge_process(
    State(state): State<Arc<AppState>>,
    Query(request): Query<FixRequest>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    let state = state.clone();

    tokio::spawn(async move {
        let mut generator = state.template_generator.lock().await;
        
        // Get temp_dir from state
        let temp_dirs = state.temp_dirs.lock().await;
        let temp_dir = match temp_dirs.get(&request.temp_dir) {
            Some(dir) => dir,
            None => {
                tx.send(ForgeStep {
                    title: "Error".to_string(),
                    output: "Session directory not found".to_string(),
                }).await.ok();
                return;
            }
        };

        // List all files in temp directory
        tx.send(ForgeStep {
            title: "Fixing".to_string(),
            output: format!("Listing files in temp dir: {:?}", 
                std::fs::read_dir(temp_dir.path())
                    .unwrap()
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .collect::<Vec<_>>()
            ),
        }).await.ok();

        let session_file = temp_dir.path().join("session.json");

        // Check if session file exists and read it
        let mut session_data = match fs::read_to_string(&session_file) {
            Ok(content) => match serde_json::from_str::<SessionData>(&content) {
                Ok(data) => data,
                Err(e) => {
                    tx.send(ForgeStep {
                        title: "Error".to_string(),
                        output: format!("Failed to parse session data: {}", e),
                    }).await.ok();
                    return;
                }
            },
            Err(e) => {
                tx.send(ForgeStep {
                    title: "Error".to_string(),
                    output: format!("Failed to read session file: {}", e),
                }).await.ok();
                return;
            }
        };

        let project_path = temp_dir.path().to_path_buf();
        let script_path = project_path.join("script").join("Script.s.sol");

        // Create script directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(script_path.parent().unwrap()) {
            tx.send(ForgeStep {
                title: "Error".to_string(),
                output: format!("Failed to create script directory: {}", e),
            })
            .await
            .ok();
            return;
        }

        match generator
            .fix_forge_code(
                temp_dir.path().to_path_buf(),
                &request.error,
                &mut session_data.messages,
                tx.clone(),
            )
            .await
        {
            Ok(fixed_code) => {
                if let Some(code) = fixed_code
                    .split("```")
                    .nth(1)
                    .and_then(|s| s.strip_prefix("solidity\n").or(Some(s)))
                {

                    fs::write(&script_path, code.trim()).unwrap();

                    // update the messages to the session file
                    if let Err(e) = fs::write(&session_file, serde_json::to_string(&session_data).unwrap()) {
                        tx.send(ForgeStep {
                            title: "Error".to_string(),
                            output: e.to_string(),
                        })
                        .await
                        .ok();
                        return;
                    }

                    let rpc_url = request
                        .rpc_url
                        .unwrap_or_else(|| "http://localhost:8545".to_string());
                    match Command::new("forge")
                        .args(&[
                            "script",
                            "script/Script.s.sol",
                            "--fork-url",
                            &rpc_url,
                            "-vvvv",
                        ])
                        .current_dir(&project_path)
                        .output()
                        .await
                    {
                        Ok(output) => {
                            // Log both stdout and stderr for debugging
                            let stdout = String::from_utf8_lossy(&output.stdout);
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            
                            tx.send(ForgeStep {
                                title: "Simulating Transactions".to_string(),
                                output: format!("STDOUT:\n{}\n\nSTDERR:\n{}", stdout, stderr),
                            })
                            .await
                            .ok();

                            // Parse successful output
                            if output.status.success() {
                                let json_path = project_path
                                    .join("broadcast")
                                    .join("Script.s.sol")
                                    .join("1")
                                    .join("dry-run")
                                    .join("run-latest.json");

                                if json_path.exists() {
                                    if let Ok(json_content) = fs::read_to_string(json_path) {
                                        if let Ok(forge_output) =
                                            serde_json::from_str::<ForgeOutput>(&json_content)
                                        {
                                            let transactions: Vec<TransactionDetails> =
                                                forge_output
                                                    .transactions
                                                    .into_iter()
                                                    .map(|tx| TransactionDetails {
                                                        to: tx.contractAddress,
                                                        function: tx.function,
                                                        arguments: tx.arguments,
                                                        value: tx.transaction.value,
                                                        input_data: tx.transaction.input,
                                                    })
                                                    .collect();

                                            tx.send(ForgeStep {
                                                title: "Simulating Transactions".to_string(),
                                                output: serde_json::to_string(&transactions)
                                                    .unwrap(),
                                            })
                                            .await
                                            .ok();
                                        } else {
                                            tx.send(ForgeStep {
                                                title: "Error".to_string(),
                                                output: "Failed to parse Forge output".to_string(),
                                            })
                                            .await
                                            .ok();
                                        }
                                    } else {
                                        tx.send(ForgeStep {
                                            title: "Error".to_string(),
                                            output: "Failed to read Forge output".to_string(),
                                        })
                                        .await
                                        .ok();
                                    }
                                }
                            } else {
                                tx.send(ForgeStep {
                                    title: "Error".to_string(),
                                    output: format!("Forge script failed:\nSTDOUT:\n{}\n\nSTDERR:\n{}", stdout, stderr),
                                })
                                .await
                                .ok();
                            }
                        }
                        Err(e) => {
                            tx.send(ForgeStep {
                                title: "Error".to_string(),
                                output: e.to_string(),
                            })
                            .await
                            .ok();
                        }
                    };
                }
            }
            Err(e) => {
                tx.send(ForgeStep {
                    title: "Error".to_string(),
                    output: e.to_string(),
                })
                .await
                .ok();
            }
        }

        // Clean up
        // if let Err(e) = std::fs::remove_dir_all(&project_path) {
        //     eprintln!("Failed to clean up fix directory: {}", e);
        // }
    });

    create_forge_stream(rx)
}

pub async fn stream_forge_process(
    State(state): State<Arc<AppState>>,
    Query(request): Query<ForgeRequest>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let session_id = request.session_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let (tx, rx) = tokio::sync::mpsc::channel(100);

    // Create and store temp dir
    let temp_dir = match TempDir::with_prefix(&format!("forge_{}_", session_id)) {
        Ok(dir) => {
            // Store TempDir in state using its path as key
            let path = dir.path().to_string_lossy().to_string();
            let mut temp_dirs = state.temp_dirs.lock().await;
            temp_dirs.insert(path.clone(), dir);

            // Send path to client
            tx.send(ForgeStep {
                title: "Session".to_string(),
                output: path.clone(),
            }).await.ok();

            PathBuf::from(path)
        }
        Err(e) => {
            tx.send(ForgeStep {
                title: "Error".to_string(),
                output: format!("Failed to create temp directory: {}", e),
            }).await.ok();
            return create_forge_stream(rx);
        }
    };

    let _permit = state.process_limiter.acquire().await.unwrap();
    let state = state.clone(); // Clone the Arc here

    tokio::spawn(async move {
        // Create session-specific temp dir

        // Use temp_dir.path() for all file operations
        let project_path = temp_dir.clone();

        tx.send(ForgeStep {
            title: "Initializing Forge".to_string(),
            output: temp_dir.as_path().to_string_lossy().to_string(),
        })
        .await
        .ok();

        // Instead of forge init, copy the base project contents
        let options = fs_extra::dir::CopyOptions::new()
            .content_only(true);  // This makes it copy only the contents

        if let Err(e) = fs_extra::dir::copy(&state.base_forge_dir, &temp_dir, &options) {
            tx.send(ForgeStep {
                title: "Error".to_string(),
                output: e.to_string(),
            })
            .await
            .ok();
            return;
        }

        let mut messages = vec![];

        let mut generator = state.template_generator.lock().await;

        let guidelines = state.protocol_processor.get_guideline(&*generator, &request.intent).await.unwrap();


        // read remappings.txt
        let remappings = fs::read_to_string(temp_dir.as_path().join("remappings.txt")).unwrap();

        // Generate code
        match generator
            .generate_forge_code(
                &request.from_address,
                &request.intent,
                &guidelines,
                &remappings,
                &mut messages,  
                tx.clone(), // Pass the sender to allow progress updates
            )
            .await
        {
            Ok(forge_code) => {
                                // Send update before parsing install commands
                tx.send(ForgeStep {
                    title: "Generating Code".to_string(),
                    output: "Saving session...".to_string() + "\n",
                })
                .await
                .ok();

                // update the messages to the session file
                let session_file = temp_dir.join("session.json");
                let session_data = SessionData {
                    messages: messages,
                };
                if let Err(e) = fs::write(&session_file, serde_json::to_string(&session_data).unwrap()) {
                    tx.send(ForgeStep {
                        title: "Error".to_string(),
                        output: e.to_string(),
                    })
                    .await
                    .ok();
                    return;
                }

                // Extract and write Solidity code
                let code = match forge_code
                    .split("```")
                    .nth(1)
                    .and_then(|s| s.strip_prefix("solidity\n").or(Some(s)))
                    .ok_or_else(|| eyre::eyre!("No Solidity code block found"))
                {
                    Ok(code) => code.to_string(),
                    Err(e) => {
                        tx.send(ForgeStep {
                            title: "Error".to_string(),
                            output: e.to_string(),
                        })
                        .await
                        .ok();
                        return;
                    }
                };

                tx.send(ForgeStep {
                    title: "Writing Code".to_string(),
                    output: "Writing code...".to_string() + "\n",
                })
                .await
                .ok();

                // List files in temp directory
                let files = match fs::read_dir(temp_dir.as_path()) {
                    Ok(entries) => {
                        let paths: Vec<_> = entries
                            .filter_map(|e| e.ok())
                            .map(|e| e.path())
                            .collect();
                        format!("Files in directory:\n{:#?}", paths)
                    },
                    Err(e) => format!("Error reading directory: {}", e)
                };

                tx.send(ForgeStep {
                    title: "Directory Contents".to_string(), 
                    output: files,
                })
                .await
                .ok();

                // Write and compile code
                let script_path = temp_dir.as_path().join("script").join("Script.s.sol");
                if let Err(e) = fs::write(&script_path, &code.trim()) {
                    tx.send(ForgeStep {
                        title: "Error".to_string(),
                        output: e.to_string(),
                    })
                    .await
                    .ok();
                    return;
                }

                tx.send(ForgeStep {
                    title: "Simulating Transactions".to_string(),
                    output: "Compiling script...".to_string() + "\n",
                })
                .await
                .ok();

                let rpc_url = request
                    .rpc_url
                    .unwrap_or_else(|| "http://localhost:8545".to_string());

                // Initial simulation
                match Command::new("forge")
                    .args(&[
                        "script",
                        "script/Script.s.sol",
                        "--fork-url",
                        &rpc_url,
                        "-vvvv",
                    ])
                    .current_dir(&project_path)
                    .output()
                    .await
                {
                    Ok(output) => {
                        // Log both stdout and stderr for debugging
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        
                        tx.send(ForgeStep {
                            title: "Simulating Transactions".to_string(),
                            output: format!("STDOUT:\n{}\n\nSTDERR:\n{}", stdout, stderr),
                        })
                        .await
                        .ok();

                        // Parse successful output
                        if output.status.success() {
                            let json_path = project_path
                                .join("broadcast")
                                .join("Script.s.sol")
                                .join("1")
                                .join("dry-run")
                                .join("run-latest.json");

                            if json_path.exists() {
                                if let Ok(json_content) = fs::read_to_string(json_path) {
                                    if let Ok(forge_output) =
                                        serde_json::from_str::<ForgeOutput>(&json_content)
                                    {
                                        let transactions: Vec<TransactionDetails> = forge_output
                                            .transactions
                                            .into_iter()
                                            .map(|tx| TransactionDetails {
                                                to: tx.contractAddress,
                                                function: tx.function,
                                                arguments: tx.arguments,
                                                value: tx.transaction.value,
                                                input_data: tx.transaction.input,
                                            })
                                            .collect();

                                        tx.send(ForgeStep {
                                            title: "Simulating Transactions".to_string(),
                                            output: serde_json::to_string(&transactions).unwrap(),
                                        })
                                        .await
                                        .ok();
                                    } else {
                                        tx.send(ForgeStep {
                                            title: "Error".to_string(),
                                            output: "Failed to parse Forge output".to_string(),
                                        })
                                        .await
                                        .ok();
                                        return;
                                    }
                                } else {
                                    tx.send(ForgeStep {
                                        title: "Error".to_string(),
                                        output: "Failed to read Forge output".to_string(),
                                    })
                                    .await
                                    .ok();
                                    return;
                                }
                            }
                        } else {
                            tx.send(ForgeStep {
                                title: "Error".to_string(),
                                output: format!("Forge script failed:\nSTDOUT:\n{}\n\nSTDERR:\n{}", stdout, stderr),
                            })
                            .await
                            .ok();
                        }
                    }
                    Err(e) => {
                        tx.send(ForgeStep {
                            title: "Error".to_string(),
                            output: e.to_string(),
                        })
                        .await
                        .ok();
                        return;
                    }
                };
            }
            Err(e) => {
                tx.send(ForgeStep {
                    title: "Error".to_string(),
                    output: e.to_string(),
                })
                .await
                .ok();
            }
        }

        // Clean up at the end
        // if let Err(e) = std::fs::remove_dir_all(&project_path) {
        //     eprintln!("Failed to clean up session {}: {}", session_id, e);
        // }

        // Permit is automatically released when _permit is dropped
    });

    create_forge_stream(rx)
}


fn create_forge_stream(
    mut rx: tokio::sync::mpsc::Receiver<ForgeStep>
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    Sse::new(stream::unfold(rx, |mut rx| async move {
        match rx.recv().await {
            Some(step) => {
                let event = Event::default().data(serde_json::to_string(&step).unwrap());
                Some((Ok(event), rx))
            }
            None => {
                // Send a final "close" event before ending the stream
                let event = Event::default()
                    .event("close")
                    .data("stream complete");
                Some((Ok(event), rx))
            }
        }
    }))
}
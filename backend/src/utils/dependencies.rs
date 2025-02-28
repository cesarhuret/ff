use crate::models::ForgeStep;
use eyre::Result;
use std::path::Path;
use tokio::process::Command;
use tokio::io::{AsyncBufReadExt, BufReader};
use std::process::Stdio;

pub async fn install_dependencies(
    project_path: &std::path::Path,
    lib_name: &str,
    tx: tokio::sync::mpsc::Sender<ForgeStep>,
) -> Result<()> {
    let lib_path = project_path.join("lib").join(lib_name);

    if lib_path.join("package.json").exists() {
        let mut child = Command::new("npm")
            .arg("install")
            .current_dir(&lib_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let tx_clone = tx.clone();
        let tx_clone2 = tx.clone();

        // Handle stdout in a separate task
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                tx_clone
                    .send(ForgeStep {
                        title: "Installing Dependencies".to_string(),
                        output: line + "\n",
                    })
                    .await
                    .ok();
            }
        });

        // Handle stderr in a separate task
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                tx_clone2
                    .send(ForgeStep {
                        title: "Installing Dependencies".to_string(),
                        output: line + "\n",
                    })
                    .await
                    .ok();
            }
        });

        // Wait for the command to complete
        child.wait().await?;
    }

    // Similar modification for forge install...
    if lib_path.join("foundry.toml").exists() || lib_path.join("remappings.txt").exists() {
        let mut child = Command::new("forge")
            .arg("install")
            .arg(&lib_name)
            .arg("--no-commit")
            .current_dir(&project_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();

        while let Ok(Some(line)) = stdout_reader.next_line().await {
            tx.send(ForgeStep {
                title: "Installing Dependencies".to_string(),
                output: line,
            })
            .await
            .ok();
        }

        while let Ok(Some(line)) = stderr_reader.next_line().await {
            tx.send(ForgeStep {
                title: "Installing Dependencies".to_string(),
                output: line,
            })
            .await
            .ok();
        }

        child.wait().await?;
    }

    Ok(())
}
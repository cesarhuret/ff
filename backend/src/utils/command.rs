use crate::models::ForgeStep;
use eyre::Result;
use tokio::process::Command;
use tokio::io::AsyncBufReadExt;
use std::process::Stdio;

pub async fn run_command_with_output(
    command: &mut Command,
    tx: &tokio::sync::mpsc::Sender<ForgeStep>,
    step_type: impl Fn(String) -> ForgeStep + Send + 'static + Clone,
) -> Result<()> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = tokio::io::BufReader::new(child.stdout.take().unwrap());
    let stderr = tokio::io::BufReader::new(child.stderr.take().unwrap());
    let tx_clone = tx.clone();
    let tx_clone2 = tx.clone();
    let step_type_stdout = step_type.clone(); // Clone for stdout task
    let step_type_stderr = step_type; // Use original for stderr task

    tokio::spawn(async move {
        let mut lines = stdout.lines();
        let mut current_progress = String::new();

        while let Ok(Some(line)) = lines.next_line().await {
            let line = line.trim();

            // If this is a progress message (contains percentage)
            if line.contains("Counting objects:")
                || line.contains("Compressing objects:")
                || line.contains("Receiving objects:")
                || line.contains("Resolving deltas:")
            {
                current_progress = line.to_string();
            } else {
                // For non-progress messages, send as normal
                tx_clone
                    .send(step_type_stdout(line.to_string() + "\n"))
                    .await
                    .ok();

                // If we had a progress message, send it now that it's complete
                if !current_progress.is_empty() {
                    let progress = current_progress.clone(); // Clone before sending
                    tx_clone.send(step_type_stdout(progress + "\n")).await.ok();
                    current_progress.clear();
                }
            }
        }
    });

    tokio::spawn(async move {
        let mut lines = stderr.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            tx_clone2.send(step_type_stderr(line + "\n")).await.ok();
        }
    });

    child.wait().await?;
    Ok(())
}
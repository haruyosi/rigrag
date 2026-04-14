use anyhow::Result;
use dialoguer::Select;
use futures::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use ollama_rs::Ollama;
use tokio::time::{Duration, interval};

pub struct ModelInfo {
    // pub support_embedding: bool,
    // pub support_completion: bool,
    // pub support_vision: bool,
    pub support_tools: bool,
    // pub support_think: bool,
}

pub async fn check_download_embedding(url: &String, model: &String) -> Result<ModelInfo> {
    let ollama = Ollama::try_new(url)?;
    let model_str = ensure_latest_tag(model);
    confirm_download(&ollama, &model_str).await?;
    let model_info = ollama.show_model_info(model_str.to_string()).await?;
    ensure_capability_or_exit(&model_str, &model_info.capabilities, "embedding");
    Ok(create_model_info(&model_info.capabilities))
}

pub async fn check_download(url: &String, model: &String, flg_picture: bool) -> Result<ModelInfo> {
    let ollama = Ollama::try_new(url)?;
    let model_str = ensure_latest_tag(model);
    confirm_download(&ollama, &model_str).await?;
    // If the --picture flag is set, also check if the model has image capabilities
    let model_info = ollama.show_model_info(model_str.clone()).await?;
    if flg_picture {
        ensure_capability_or_exit(&model_str, &model_info.capabilities, "vision");
    }
    ensure_capability_or_exit(&model_str, &model_info.capabilities, "completion");
    Ok(create_model_info(&model_info.capabilities))
}

fn create_model_info(capabilities: &[String]) -> ModelInfo {
    ModelInfo {
        // support_embedding: capabilities.iter().any(|c| c == "embedding"),
        // support_completion: capabilities.iter().any(|c| c == "completion"),
        // support_vision: capabilities.iter().any(|c| c == "vision"),
        support_tools: capabilities.iter().any(|c| c == "tools"),
        // support_think: capabilities.iter().any(|c| c == "think"),
    }
}

fn ensure_latest_tag(model_name: &str) -> String {
    if model_name.contains(':') {
        model_name.to_string()
    } else {
        format!("{model_name}:latest")
    }
}

fn ensure_capability_or_exit(model_name: &str, capabilities: &[String], capability: &str) {
    if capabilities.iter().any(|c| c == capability) {
        return;
    }
    println!(
        "Model '{}' does not have {} capabilities.",
        model_name, capability
    );
    std::process::exit(0);
}

async fn confirm_download(ollama: &Ollama, model_name: &str) -> Result<()> {
    let local_models = ollama.list_local_models().await?;
    let is_model_available = local_models.iter().any(|m| m.name == model_name);
    if is_model_available {
        return Ok(());
    }

    let choice = Select::new()
        .with_prompt(format!(
            "Model '{}' is not available. Do you want to download it to Ollama?",
            model_name
        ))
        .default(0)
        .item("Yes")
        .item("No")
        .interact()?;

    match choice {
        0 => {
            println!("Starting download of model '{}' to Ollama.", model_name);
        }
        1 => {
            println!("Model '{}' is required. Exiting.", model_name);
            std::process::exit(0);
        }
        _ => unreachable!(),
    }

    let mut pull_model_status = ollama
        .pull_model_stream(model_name.to_string(), false)
        .await?;
    let mut ticker = interval(Duration::from_secs(1));

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner} downloading model... [{elapsed_precise}]")
            .unwrap(),
    );

    ticker.tick().await;
    loop {
        tokio::select! {
            maybe_status = pull_model_status.next() => {
                match maybe_status {
                    Some(Ok(status)) => {
                        if status.message == "success" {
                            println!("Model '{}' download completed.", model_name);
                            break;
                        }
                    }
                    Some(Err(e)) => {
                        eprintln!("Error while downloading model '{}': {}", model_name, e);
                        std::process::exit(1);
                    }
                    None => {
                        println!("Model '{}' download stream ended.", model_name);
                        break;
                    }
                }
            }
            _ = ticker.tick() => {
                spinner.tick();
            }
        }
    }

    spinner.finish_and_clear();
    Ok(())
}

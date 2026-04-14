use anyhow::Result;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sys_locale::get_locale;

use crate::{args::Args, chat, reader};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub url: String,
    pub embed_model: String,
    pub embed_thread: u32,
    pub single_thread: bool,
    pub image: reader::ImageDescriptionConfig,
    pub chat: chat::ChatConfig,
    pub exclude: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let locale = get_locale().unwrap_or_else(|| "en-US".to_string());
        let system_chat = match &locale[0..2] {
            "ja" => include_str!("./system/system_chat_ja.md"),
            "en" => include_str!("./system/system_chat_en.md"),
            _ => include_str!("./system/system_chat_en.md"),
        };
        let system_image = match &locale[0..2] {
            "ja" => include_str!("./system/system_image_ja.md"),
            "en" => include_str!("./system/system_image_en.md"),
            _ => include_str!("./system/system_image_en.md"),
        };
        Self {
            url: "http://localhost:11434".to_string(),
            embed_model: "nomic-embed-text-v2-moe".to_string(),
            embed_thread: 4,
            single_thread: false,
            image: reader::ImageDescriptionConfig {
                is_need: false,
                model: "qwen3.5".to_string(),
                system: system_image.to_string(),
                temperature: 0.2,
                url: "http://localhost:11434".to_string(),
            },
            chat: chat::ChatConfig {
                model: "qwen3.5".to_string(),
                system: system_chat.to_string(),
                temperature: 0.2,
                sample: 5,
                verbose: false,
                url: "http://localhost:11434".to_string(),
            },
            exclude: Vec::new(),
        }
    }
}

pub async fn open_config(path: &PathBuf, args: &Args) -> Result<AppConfig> {
    let mut cfg: AppConfig = confy::load_path(path)?;
    cfg.url = args.url.clone().unwrap_or(cfg.url);
    cfg.embed_thread = args.embedding_thread.unwrap_or(cfg.embed_thread);
    cfg.single_thread = args.single_thread;
    cfg.image.is_need = args.picture;
    cfg.image.model = args.model.clone().unwrap_or(cfg.image.model);
    cfg.image.temperature = args.temperature.unwrap_or(cfg.image.temperature);
    cfg.image.url = args.url.clone().unwrap_or(cfg.image.url);
    cfg.chat.model = args.model.clone().unwrap_or(cfg.chat.model);
    cfg.chat.temperature = args.temperature.unwrap_or(cfg.chat.temperature);
    cfg.chat.sample = args.sample.unwrap_or(cfg.chat.sample);
    cfg.chat.verbose = args.verbose;
    cfg.chat.url = args.url.clone().unwrap_or(cfg.chat.url);
    cfg.exclude = args.exclude.clone().unwrap_or(cfg.exclude);

    confy::store_path(path, &cfg)?;
    Ok(cfg)
}

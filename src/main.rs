use anyhow::Result;
use clap::Parser;
use rig::client::{CompletionClient, EmbeddingsClient, Nothing};
use rig::completion::Prompt;
use rig::providers::ollama::Client;
use rig_lancedb::{LanceDbVectorIndex, SearchParams};
use std::io::IsTerminal;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, stdin};

use crate::db_store::LanceDbDocumentStore;
use crate::tools::create_rig_tools;

mod args;
mod chat;
mod crawler;
mod db_store;
mod models;
mod reader;
mod tools;
mod conf;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let is_tty = std::io::stdin().is_terminal();
    // Parse command-line arguments
    let mut args = args::Args::parse();
    if !is_tty {
        let mut buf = String::new();
        stdin().read_to_string(&mut buf).await.unwrap();
        args.query = Some(buf);
    }
    let target_dir = PathBuf::from(&args.path);
    // Check if the specified path exists
    if target_dir.is_file() {
        println!("Specified path is a file, not a directory: {}", args.path);
        std::process::exit(1);
    }
    if !target_dir.exists() {
        println!("Target directory does not exist, creating: {}", args.path);
        std::process::exit(1);
    }

    let app_path = target_dir.join(".rigrag");
    let config_path = app_path.join("config.toml");

    let cfg = conf::open_config(&config_path, &args).await?;

    models::check_download_embedding(&cfg.url, &cfg.embed_model).await?;
    let model_info = models::check_download(&cfg.url, &cfg.chat.model, cfg.image.is_need).await?;

    let ollama_client = Client::builder()
        .api_key(Nothing)
        .base_url(&cfg.url)
        .build()?;
    let embed_model_name = &cfg.embed_model;
    let embed_model = ollama_client.embedding_model(embed_model_name);

    let db_path = app_path.join("lancedb-store");
    let db = lancedb::connect(&db_path.display().to_string())
        .execute()
        .await?;
    let mut table_name = String::from("documentTbl_");
    table_name.push_str(embed_model_name);
    let is_table_exsist = LanceDbDocumentStore::is_exsist_table(&db, &table_name).await?;
    if args.init || !is_table_exsist {
        // if the --init flag is set, clear the existing data and re-ingest the documents
        crawler::crawl(&cfg, target_dir, embed_model.clone(), &db).await?;
    }
    let table = LanceDbDocumentStore::open_table(&db, &table_name).await?;
    let index_db =
        LanceDbVectorIndex::new(table, embed_model, "doc", SearchParams::default()).await?;

    // If a query is provided, perform a single retrieval and response generation; otherwise, start the interactive chat loop
    if let Some(query) = args.query {
        let tool_table = LanceDbDocumentStore::open_table(&db, &table_name).await?;
        let tool_embed = ollama_client.embedding_model(embed_model_name);
        let tool_index = Arc::new(
            LanceDbVectorIndex::new(tool_table, tool_embed, "doc", SearchParams::default()).await?,
        );
        let tools = create_rig_tools(tool_index);
        println!("### support_tools: {}", model_info.support_tools);
        let rag_agent = if model_info.support_tools {
            ollama_client
                .agent(&cfg.chat.model)
                .preamble(&cfg.chat.system)
                .dynamic_context(cfg.chat.sample as usize, index_db)
                .temperature(cfg.chat.temperature as f64)
                .tools(tools)
                .default_max_turns(8)
                .build()
        } else {
            ollama_client
                .agent(&cfg.chat.model)
                .preamble(&cfg.chat.system)
                .dynamic_context(cfg.chat.sample as usize, index_db)
                .temperature(cfg.chat.temperature as f64)
                .default_max_turns(8)
                .build()
        };
        let response = rag_agent.prompt(query).await?;
        println!("{response}");
    } else {
        chat::chat(&cfg.chat, &index_db).await?;
    }
    Ok(())
}

use anyhow::Result;
use futures::StreamExt;
use ollama_rs::{
    Ollama,
    generation::{
        chat::{ChatMessage, request::ChatMessageRequest},
        parameters::ThinkType,
        tools::ToolCall,
    },
    models::ModelOptions,
};
use rig::{
    providers::ollama::EmbeddingModel,
    vector_store::{VectorSearchRequest, VectorStoreIndex},
};
use rig_lancedb::LanceDbVectorIndex;
use serde::{Deserialize, Serialize};
use std::io::{Write, stdin, stdout};

use crate::{
    reader::ChunkedDocument,
    tools::{
        ToolResult, adder, average, create_ollama_tools, divide, multiply, subtract, sum, today,
    },
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatConfig {
    pub model: String,
    pub system: String,
    pub temperature: f32,
    pub sample: u64,
    pub verbose: bool,
    pub url: String,
}

pub async fn chat(
    config: &ChatConfig,
    index_db: &LanceDbVectorIndex<EmbeddingModel>,
) -> Result<()> {
    let ollama = Ollama::try_new(&config.url)?;
    let model: &String = &config.model;

    let model_info = ollama.show_model_info(model.clone()).await?;
    let opt_tool = model_info.capabilities.iter().find(|&c| c == "tools");

    // 1. 会話履歴を保持するベクタを用意（最初のシステムコマンドを入れておく）
    let mut history: Vec<ChatMessage> = vec![ChatMessage::system(config.system.clone())];

    let tools = create_ollama_tools().await;

    // let mut flg_show_score = true;

    println!("--- if type 'exit' to quit, 'clear' to clear history ---");
    println!();
    let mut input = String::new();
    let mut tool_call: Option<ToolCall> = None;
    let mut db_refs = Vec::new();

    loop {
        let mut full_response = String::new();
        // let mut final_tool_calls = None;

        match tool_call {
            Some(ref call) => {
                if config.verbose {
                    println!("function: {}", call.function.name);
                    println!("arguments: {:?}", call.function.arguments);
                }
                match call.function.name.as_str() {
                    "search" => {
                        let tool_input = call
                            .function
                            .arguments
                            .get("query")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        println!("Searching DB. \"{}\"", tool_input);
                        let index_results = search_db(&tool_input, config, index_db).await?;
                        for (_, text, _, _) in &index_results {
                            history.push(ChatMessage::tool(format!("search result: {}", text)));
                        }
                    }
                    "adder" => {
                        let result = adder::Adder::new()
                            .add(get_arg_f64(call, "a"), get_arg_f64(call, "b"))
                            .to_string();
                        push_tool_result(&mut history, &call.function.name, result)?;
                    }
                    "subtract" => {
                        let result = subtract::Subtract::new()
                            .subtract(get_arg_f64(call, "a"), get_arg_f64(call, "b"))
                            .to_string();
                        push_tool_result(&mut history, &call.function.name, result)?;
                    }
                    "multiply" => {
                        let result = multiply::Multiply::new()
                            .multiply(get_arg_f64(call, "a"), get_arg_f64(call, "b"))
                            .to_string();
                        push_tool_result(&mut history, &call.function.name, result)?;
                    }
                    "divide" => {
                        let result = divide::Divide::new()
                            .divide(get_arg_f64(call, "a"), get_arg_f64(call, "b"))
                            .to_string();
                        push_tool_result(&mut history, &call.function.name, result)?;
                    }
                    "sum" => {
                        let result = sum::Sum::new()
                            .sum(get_arg_f64_list(call, "elements"))
                            .to_string();
                        push_tool_result(&mut history, &call.function.name, result)?;
                    }
                    "average" => {
                        let result = average::Average::new()
                            .average(get_arg_f64_list(call, "elements"))
                            .to_string();
                        push_tool_result(&mut history, &call.function.name, result)?;
                    }
                    "today" => {
                        let result = today::Today::new().today();
                        push_tool_result(&mut history, &call.function.name, result)?;
                    }
                    _ => println!("Unknown tool function: {}", call.function.name),
                }
                tool_call = None;
            }
            None => {
                input.clear();
                print!("{} >> ", model);
                stdout().flush()?;
                stdin().read_line(&mut input)?;
                input = input.trim().to_string();
                println!();

                if input == "exit" {
                    break;
                }

                if input == "clear" {
                    history.clear();
                    history.push(ChatMessage::system(config.system.clone()));
                    input = "/clear".to_string();
                    continue;
                }
                if input.is_empty() {
                    continue;
                }

                let index_results = search_db(&input, config, index_db).await?;
                db_refs.extend(index_results.clone());
                // Add search results to conversation history for the model
                for (_, text, _, _) in &index_results {
                    history.push(ChatMessage::tool(format!("search result: {}", text)));
                }
                // 2. ユーザーの発言を履歴に追加
                history.push(ChatMessage::user(input.to_string()));
            }
        }

        // 3. リクエスト作成（履歴全体を渡す）
        let request = match opt_tool {
            Some(_) => ChatMessageRequest::new(model.clone(), history.clone())
                .options(ModelOptions::default().temperature(config.temperature as f32))
                .think(ThinkType::High)
                .tools(tools.clone()),
            None => ChatMessageRequest::new(model.clone(), history.clone())
                .think(ThinkType::High)
                .options(ModelOptions::default().temperature(config.temperature as f32)),
        };
        let mut stream = ollama.send_chat_messages_stream(request).await?;

        stdout().flush()?;

        // 4. ストリーミングで回答を受け取る
        while let Some(res) = stream.next().await {
            match res {
                Ok(res) => {
                    // レスポンスにツール呼び出しがあるか確認
                    for call in res.message.tool_calls {
                        if is_supported_tool(call.function.name.as_str()) {
                            tool_call = Some(call);
                        } else {
                            println!("Unknown tool function: {}", call.function.name);
                        }
                    }
                    // LLMからの回答を表示
                    let content = &res.message.content;
                    print!("{}", content);
                    stdout().flush()?;
                    full_response.push_str(content);
                }
                Err(_e) => {
                    eprintln!("Error occurred while streaming");
                    break;
                }
            };
        }

        match tool_call {
            Some(_) => {}
            None => {
                db_refs.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap());
                db_refs.dedup_by_key(|a| a.3.chars().next());
                println!();
                println!("----------------------------------------------");
                // show up references, sorted by chapter (assuming chapter indicates relevance or order)
                for (_, text, path, chapter) in &db_refs {
                    println!();
                    println!("path: {}", path);
                    if !chapter.is_empty() {
                        println!("chapter: {}", chapter);
                    }
                    if config.verbose {
                        println!("{}", text);
                    }
                }
                db_refs.clear();
            }
        }
        // add assistant response to history
        history.push(ChatMessage::assistant(full_response));
    }
    Ok(())
}

async fn search_db(
    input: &str,
    config: &ChatConfig,
    index_db: &LanceDbVectorIndex<EmbeddingModel>,
) -> Result<Vec<(f64, String, String, String)>> {
    // create vector search request
    let req = VectorSearchRequest::builder()
        .query(input)
        .samples(config.sample)
        .build()?;

    // Get search results from vector store index
    let results = index_db
        .top_n::<ChunkedDocument>(req.clone())
        .await?
        .into_iter()
        .map(|(score, text, doc)| (score, text, doc.path, doc.chapter))
        .collect::<Vec<_>>();
    Ok(results)
}

fn get_arg_f64(call: &ToolCall, key: &str) -> f64 {
    call.function
        .arguments
        .get(key)
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0)
}

fn get_arg_f64_list(call: &ToolCall, key: &str) -> Vec<f64> {
    call.function
        .arguments
        .get(key)
        .and_then(|v| v.as_array())
        .map(|values| values.iter().filter_map(|v| v.as_f64()).collect())
        .unwrap_or_default()
}

fn push_tool_result(
    history: &mut Vec<ChatMessage>,
    tool_name: &str,
    content: String,
) -> Result<()> {
    let tool_result = ToolResult {
        role: "tool".to_string(),
        tool_call_id: tool_name.to_string(),
        content,
    };
    history.push(ChatMessage::tool(serde_json::to_string(&tool_result)?));
    Ok(())
}

fn is_supported_tool(name: &str) -> bool {
    matches!(
        name,
        "search" | "adder" | "subtract" | "multiply" | "divide" | "sum" | "average" | "today"
    )
}

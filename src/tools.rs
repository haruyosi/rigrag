use std::sync::Arc;

use ollama_rs::generation::tools::{ToolFunctionInfo, ToolInfo, ToolType};
use rig::{providers::ollama::EmbeddingModel, tool::ToolDyn};
use rig_lancedb::LanceDbVectorIndex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod adder;
pub mod average;
pub mod divide;
pub mod multiply;
pub mod search;
pub mod subtract;
pub mod sum;
pub mod today;

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct ToolResult {
    pub role: String,
    pub tool_call_id: String,
    pub content: String,
}

fn create_stateless_rig_tools() -> Vec<Box<dyn ToolDyn>> {
    vec![
        Box::new(adder::Adder::new()),
        Box::new(subtract::Subtract::new()),
        Box::new(multiply::Multiply::new()),
        Box::new(divide::Divide::new()),
        Box::new(sum::Sum::new()),
        Box::new(average::Average::new()),
        Box::new(today::Today::new()),
    ]
}

pub fn create_rig_tools(
    index_db: Arc<LanceDbVectorIndex<EmbeddingModel>>,
) -> Vec<Box<dyn ToolDyn>> {
    let mut tools = vec![Box::new(search::Search::new(index_db)) as Box<dyn ToolDyn>];
    tools.extend(create_stateless_rig_tools());
    tools
}

pub async fn create_ollama_tools() -> Vec<ToolInfo> {
    let mut tool_info = vec![ToolInfo {
        tool_type: ToolType::Function,
        function: serde_json::from_str::<ToolFunctionInfo>(include_str!("./tools/search.json"))
            .unwrap(),
    }];
    for tool in create_stateless_rig_tools().iter() {
        tool_info.push(rig_tool_to_ollama_tool(tool).await);
    }
    tool_info
}

async fn rig_tool_to_ollama_tool(tool: &Box<dyn ToolDyn>) -> ToolInfo {
    let def = tool.definition("".to_string()).await;
    let json = serde_json::to_string(&def).unwrap();
    ToolInfo {
        tool_type: ToolType::Function,
        function: serde_json::from_str(&json).unwrap(),
    }
}

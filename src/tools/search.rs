use rig::completion::ToolDefinition;
use rig::providers::ollama::EmbeddingModel;
use rig::tool::Tool;
use rig::vector_store::{VectorSearchRequest, VectorStoreIndex};
use rig_lancedb::LanceDbVectorIndex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::reader::ChunkedDocument;

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
pub struct SearchArg {
    query: String,
    samples: usize,
}

#[derive(Debug, thiserror::Error)]
#[error("Search error")]
pub struct SearchError;

pub struct Search {
    index_db: Arc<LanceDbVectorIndex<EmbeddingModel>>,
}
impl Search {
    pub fn new(index_db: Arc<LanceDbVectorIndex<EmbeddingModel>>) -> Self {
        Self { index_db }
    }
}

impl Tool for Search {
    const NAME: &'static str = "search";
    type Error = SearchError;
    type Args = SearchArg;
    type Output = String;
    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Search the indexed documents with a query. Args: {query: string}"
                .to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(SearchArg)).unwrap(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!(
            "SearchTool called with samples: {} query: {}",
            args.samples, args.query
        );
        let req = VectorSearchRequest::builder()
            .query(args.query.clone())
            .samples(args.samples as u64)
            .build()
            .map_err(|_| SearchError)?;
        let results = self
            .index_db
            .top_n::<ChunkedDocument>(req)
            .await
            .map_err(|_| SearchError)?;

        Ok(results
            .into_iter()
            .map(|(score, text, doc)| {
                format!(
                    "Score: {:.4}, Path: {}, Chapter: {}, Text: {}",
                    score, doc.path, doc.chapter, text
                )
            })
            .collect::<Vec<_>>()
            .join("\n"))
    }
}

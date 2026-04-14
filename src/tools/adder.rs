use rig::completion::ToolDefinition;
use rig::tool::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
pub struct AdderArg {
    x: f64,
    y: f64,
}

#[derive(Debug, thiserror::Error)]
#[error("Adder error")]
pub struct AdderError;

pub struct Adder;

impl Adder {
    pub fn new() -> Self {
        Self
    }

    pub fn add(&self, x: f64, y: f64) -> f64 {
        let result = x + y;
        println!(
            "Tool Adder called with x: {} y: {} result: {}",
            x, y, result
        );
        result
    }
}

impl Tool for Adder {
    const NAME: &'static str = "adder";
    type Error = AdderError;
    type Args = AdderArg;
    type Output = f64;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Add x and y together".to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(AdderArg)).unwrap(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let result = self.add(args.x, args.y);
        Ok(result)
    }
}

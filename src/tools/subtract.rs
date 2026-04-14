use rig::completion::ToolDefinition;
use rig::tool::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
pub struct SubtractArg {
    x: f64,
    y: f64,
}

#[derive(Debug, thiserror::Error)]
#[error("Subtract error")]
pub struct SubtractError;

pub struct Subtract;

impl Subtract {
    pub fn new() -> Self {
        Self
    }

    pub fn subtract(&self, x: f64, y: f64) -> f64 {
        let result = x - y;
        println!(
            "Tool Subtract called with x: {} y: {} result: {}",
            x, y, result
        );
        result
    }
}

impl Tool for Subtract {
    const NAME: &'static str = "subtract";
    type Error = SubtractError;
    type Args = SubtractArg;
    type Output = f64;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Subtract y from x".to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(SubtractArg)).unwrap(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let result = self.subtract(args.x, args.y);
        Ok(result)
    }
}

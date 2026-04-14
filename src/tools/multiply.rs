use rig::completion::ToolDefinition;
use rig::tool::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
pub struct MultiplyArg {
    x: f64,
    y: f64,
}

#[derive(Debug, thiserror::Error)]
#[error("Multiply error")]
pub struct MultiplyError;

pub struct Multiply;

impl Multiply {
    pub fn new() -> Self {
        Self
    }

    pub fn multiply(&self, x: f64, y: f64) -> f64 {
        let result = x * y;
        println!(
            "Tool Multiply called with x: {} y: {} result: {}",
            x, y, result
        );
        result
    }
}

impl Tool for Multiply {
    const NAME: &'static str = "multiply";
    type Error = MultiplyError;
    type Args = MultiplyArg;
    type Output = f64;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Multiply x and y together".to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(MultiplyArg)).unwrap(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let result = self.multiply(args.x, args.y);
        Ok(result)
    }
}

use rig::completion::ToolDefinition;
use rig::tool::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
pub struct DivideArg {
    x: f64,
    y: f64,
}

#[derive(Debug, thiserror::Error)]
#[error("Divide error")]
pub struct DivideError;

pub struct Divide;

impl Divide {
    pub fn new() -> Self {
        Self
    }

    pub fn divide(&self, x: f64, y: f64) -> f64 {
        if y == 0.0 {
            return 0.0; // or you could choose to panic or return an error
        }
        let result = x / y;
        println!(
            "Tool Divide called with x: {} y: {} result: {}",
            x, y, result
        );
        result
    }
}

impl Tool for Divide {
    const NAME: &'static str = "divide";
    type Error = DivideError;
    type Args = DivideArg;
    type Output = f64;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Divide x by y".to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(DivideArg)).unwrap(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let result = self.divide(args.x, args.y);
        Ok(result)
    }
}

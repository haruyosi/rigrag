use rig::completion::ToolDefinition;
use rig::tool::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
pub struct SumArg {
    elements: Vec<f64>,
}

#[derive(Debug, thiserror::Error)]
#[error("Sum error")]
pub struct SumError;

pub struct Sum;

impl Sum {
    pub fn new() -> Self {
        Self
    }

    pub fn sum(&self, elements: Vec<f64>) -> f64 {
        let result: f64 = elements.iter().sum();
        println!(
            "Tool Sum called with elements: {:?} result: {}",
            elements, result
        );
        result
    }
}

impl Tool for Sum {
    const NAME: &'static str = "sum";
    type Error = SumError;
    type Args = SumArg;
    type Output = f64;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Sum all elements in the list".to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(SumArg)).unwrap(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let result = self.sum(args.elements);
        Ok(result)
    }
}

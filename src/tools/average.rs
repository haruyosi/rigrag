use rig::completion::ToolDefinition;
use rig::tool::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
pub struct AverageArg {
    elements: Vec<f64>,
}

#[derive(Debug, thiserror::Error)]
#[error("Average error")]
pub struct AverageError;

pub struct Average;

impl Average {
    pub fn new() -> Self {
        Self
    }

    pub fn average(&self, elements: Vec<f64>) -> f64 {
        let sum: f64 = elements.iter().sum();
        let count = elements.len() as f64;
        let result = if count > 0.0 { sum / count } else { 0.0 };
        println!(
            "Tool Average called with elements: {:?} result: {}",
            elements, result
        );
        result
    }
}

impl Tool for Average {
    const NAME: &'static str = "average";
    type Error = AverageError;
    type Args = AverageArg;
    type Output = f64;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Calculate the average of all elements in the list".to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(AverageArg)).unwrap(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let result = self.average(args.elements);
        Ok(result)
    }
}

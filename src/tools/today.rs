use chrono::Local;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
pub struct TodayArg {}

#[derive(Debug, thiserror::Error)]
#[error("Today error")]
pub struct TodayError;

pub struct Today;

impl Today {
    pub fn new() -> Self {
        Self
    }

    pub fn today(&self) -> String {
        let today = Local::now().format("%Y-%m-%d").to_string();
        println!("Tool Today called result: {}", today);
        today
    }
}

impl Tool for Today {
    const NAME: &'static str = "today";
    type Error = TodayError;
    type Args = TodayArg;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Get today's date in YYYY-MM-DD format".to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(TodayArg)).unwrap(),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        let result = self.today();
        Ok(result)
    }
}

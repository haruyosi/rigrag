use clap::Parser;

#[derive(Parser, Debug, Clone, Default)]
#[command(version, about)]
pub struct Args {
    /// Target path to index
    #[arg(default_value = ".")]
    pub path: String,

    /// Query
    #[arg()]
    pub query: Option<String>,

    /// Initialize db index
    #[arg(short, long, default_value_t = false)]
    pub init: bool,

    /// Include pictures in the processing (experimental)
    #[arg(short, long, default_value_t = false)]
    pub picture: bool,

    /// Ollama API URL
    #[arg(short, long, default_value = "http://localhost:11434")]
    pub url: Option<String>,

    /// Embedding model to use
    #[arg(short, long, default_value = "nomic-embed-text-v2-moe")]
    pub embedding_model: Option<String>,

    /// Number of threads for embedding
    #[arg(long, default_value = "4")]
    pub embedding_thread: Option<u32>,

    /// LLM model to use for generation
    #[arg(short, long, default_value = "qwen3.5")]
    pub model: Option<String>,

    /// Search sample size
    #[arg(short, long)]
    pub sample: Option<u64>,

    /// Temperature for LLM generation
    #[arg(short, long)]
    pub temperature: Option<f32>,

    /// Verbose mode
    #[arg(short, long, default_value_t = false)]
    pub verbose: bool,

    /// Single-thread mode
    #[arg(long, default_value_t = false)]
    pub single_thread: bool,

    /// Exclude file extensions (comma-separated)
    #[arg(short='x', long, value_delimiter = ',', num_args = 1..)]
    pub exclude: Option<Vec<String>>,
}

use ollama_rs::Ollama;
use ollama_rs::generation::completion::GenerationResponse;
use ollama_rs::generation::completion::request::GenerationRequest;
use ollama_rs::generation::images::Image;
use ollama_rs::generation::parameters::ThinkType;
use ollama_rs::models::ModelOptions;
use rig::Embed;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::path::Path;
use std::sync::OnceLock;
use text_splitter::{ChunkConfig, MarkdownSplitter, TextSplitter};
use tokenizers::Tokenizer;
use tokio::runtime::Builder;

pub mod epub;
pub mod excel;
pub mod excel_old;
pub mod file_list;
pub mod java;
pub mod markdown;
pub mod pdf;
pub mod powerpoint;
pub mod text;
pub mod word;

pub const CHUNK_SIZE: usize = 512;

static TOKENIZER: OnceLock<Tokenizer> = OnceLock::new();
static MARKDOWN_SPLITTER: OnceLock<MarkdownSplitter<Tokenizer>> = OnceLock::new();
static TEXT_SPLITTER: OnceLock<TextSplitter<Tokenizer>> = OnceLock::new();
pub const PROGRESSBAR_BLUE: &str = "{spinner:.yellow} [{eta_precise}] {percent:>3}% {bar:30.blue/cyan} {pos}/{len} ({per_sec}) {msg}";
pub const PROGRESSBAR_GREEN: &str = "{spinner:.yellow} [{eta_precise}] {percent:>3}% {bar:30.green/green} {pos}/{len} ({per_sec}) {msg}";
pub const PROGRESSBAR_YELLOW: &str = "{spinner:.yellow} [{eta_precise}] {percent:>3}% {bar:30.yellow/yellow} {pos}/{len} ({per_sec}) {msg}";

// Shape of data that needs to be RAG'ed.
// The definition field will be used to generate embeddings.
#[derive(Embed, Clone, Deserialize, Debug, Serialize, Eq, PartialEq, Default)]
pub struct ChunkedDocument {
    pub path: String,
    pub chapter: String,
    #[embed]
    pub doc: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ImageDescriptionConfig {
    pub is_need: bool,
    pub model: String,
    pub system: String,
    pub temperature: f32,
    pub url: String,
}

impl Display for ChunkedDocument {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(
            f,
            "-----\npath: {}\nchapter: {}\ndoc: {}",
            self.path, self.chapter, self.doc
        )
    }
}

pub trait Reader {
    fn read(
        &self,
        target_dir: &Path,
        entry: &file_list::EntryInfo,
    ) -> Result<Vec<ChunkedDocument>, String>;
}

fn get_tokenizer() -> &'static Tokenizer {
    TOKENIZER.get_or_init(|| {
        Tokenizer::from_pretrained("bert-base-cased", None)
            .unwrap_or_else(|e| panic!("Failed to load tokenizer: {}", e))
    })
}

fn create_chunk_config() -> ChunkConfig<Tokenizer> {
    ChunkConfig::new(CHUNK_SIZE)
        .with_sizer(get_tokenizer().clone())
        .with_trim(true)
        .with_overlap(CHUNK_SIZE / 3)
        .unwrap_or_else(|e| panic!("Failed to create chunk config: {}", e))
}

fn split_text(text: &str) -> Vec<String> {
    MARKDOWN_SPLITTER
        .get_or_init(|| MarkdownSplitter::new(create_chunk_config()))
        .chunks(text)
        .map(str::to_string)
        .collect()
}

fn split_code(text: &str) -> Vec<String> {
    TEXT_SPLITTER
        .get_or_init(|| TextSplitter::new(create_chunk_config()))
        .chunks(text)
        .map(str::to_string)
        .collect()
}

fn make_document(
    // reader: &impl Reader,
    fileinfo: &str,
    paragraph: &str,
    content: &str,
) -> Result<ChunkedDocument, String> {
    let doc = ChunkedDocument {
        path: fileinfo.to_string(),
        chapter: paragraph.to_string(),
        doc: format!("{}\n{}", paragraph, content),
    };
    // println!("path: {}, chapter: {}", doc.path, doc.chapter);
    Ok(doc)
}

fn describe_image(
    config: &ImageDescriptionConfig,
    image_base64: &str,
    content: &str,
) -> Result<String, String> {
    if image_base64.trim().is_empty() {
        return Ok(String::new());
    }

    let request = GenerationRequest::new(
        config.model.to_string(),
        format!("context: {}\n prompt: {}", content, config.system),
    )
    .add_image(Image::from_base64(image_base64))
    .think(ThinkType::False)
    .system(config.system.to_string())
    .options(ModelOptions::default().temperature(config.temperature as f32));
    let runtime = match Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("Failed to create runtime for image analysis: {}", e);
            return Ok(String::new());
        }
    };

    let response = runtime
        .block_on(async {
            let ollama = Ollama::try_new(&config.url).unwrap();
            ollama.generate(request).await
        })
        .unwrap_or(GenerationResponse {
            model: config.model.to_string(),
            created_at: "".to_string(),
            response: "".to_string(),
            done: true,
            context: None,
            total_duration: None,
            load_duration: None,
            prompt_eval_count: None,
            prompt_eval_duration: None,
            eval_count: None,
            eval_duration: None,
            thinking: None,
            logprobs: None,
        });
    // println!("-----------------------------\n{}", response.response);
    Ok(response.response)
}

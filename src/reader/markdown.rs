use base64::{Engine, prelude::BASE64_STANDARD};
use encoding_rs_io::DecodeReaderBytesBuilder;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use lazy_static::lazy_static;
use regex::{Captures, Regex};

use crate::reader::{
    self, ChunkedDocument, ImageDescriptionConfig, Reader, describe_image, file_list,
    make_document, split_text,
};
use std::{
    fs::{self, File},
    io::{BufRead, BufReader},
    path::Path,
    sync::Arc,
};

lazy_static! {
    static ref HEADING_REGEX: Regex = Regex::new(r"^#{1,6}\s+(.*)").unwrap();
    static ref MD_IMG_REGEX: Regex = Regex::new(r"\!\[.*\]\((.*)\)").unwrap();
    static ref ADOC_IMG_REGEX: Regex = Regex::new(r"image::(.*)\[.*\]").unwrap();
    static ref ADOC_IMAGESDIR_REGEX: Regex = Regex::new(r"^:imagesdir:\s+(.*)$").unwrap();
}

pub struct MarkdownReader {
    config: ImageDescriptionConfig,
    mp: Arc<MultiProgress>,
}

impl MarkdownReader {
    pub fn new(config: ImageDescriptionConfig, mp: Arc<MultiProgress>) -> Self {
        Self { config, mp }
    }

    fn description_image(
        &self,
        caps: Captures<'_>,
        file_path: &Path,
        target_dir: &Path,
        content: &str,
    ) -> String {
        let mut res = String::new();
        let image_base64;
        if let Some(m) = caps.get(1) {
            let img_file = m.as_str().to_string();
            if img_file.starts_with("http") {
                return String::new(); // URLの場合はスキップ
            }
            let base: &str;
            if img_file.starts_with(".") {
                base = file_path
                    .parent()
                    .unwrap_or_else(|| Path::new(""))
                    .to_str()
                    .unwrap_or("");
            } else {
                base = target_dir.to_str().unwrap_or("");
            }
            let img_path = format!("{}{}", base, img_file);
            if fs::metadata(&img_path).is_ok() {
                // println!("### Found image: {}", img_path); // デバッグ用に見つけた画像を出力
                let image_bytes = fs::read(&img_path)
                    .map_err(|e| format!("Failed to read image file {}: {e}", img_path))
                    .unwrap_or_default();
                image_base64 = BASE64_STANDARD.encode(&image_bytes);
                res = describe_image(&self.config, &image_base64, content).unwrap_or(String::new());
            }
        }
        res
    }
}

impl Reader for MarkdownReader {
    fn read(
        &self,
        target_dir: &Path,
        entry: &file_list::EntryInfo,
    ) -> Result<Vec<ChunkedDocument>, String> {
        let suffixes = vec!["md", "adoc"];
        let mut res = Vec::new();
        if !suffixes.contains(&entry.suffix.to_lowercase().as_str()) {
            return Ok(res);
        }
        let mut paragraph_str = String::new();
        let file_path = target_dir.join(&entry.path);
        let file_path_str = file_path
            .to_str()
            .ok_or_else(|| "Invalid file path".to_string())?
            .to_string();

        let file = File::open(&file_path)
            .map_err(|e| format!("Failed to open file {}: {e}", file_path_str))?;
        let decoder = DecodeReaderBytesBuilder::new().build(file);
        let reader = BufReader::new(decoder);
        let mut count = 0;
        for (index, line) in reader.lines().enumerate() {
            match line {
                Ok(line) => {
                    if MD_IMG_REGEX.captures(&line).is_some()
                        || ADOC_IMG_REGEX.captures(&line).is_some()
                    {
                        // 画像の説明は後でまとめて行うため、ここではスキップ
                        count += 1;
                    }
                }
                Err(e) => eprintln!("Error at line {}: {}", index + 1, e),
            }
        }
        let desc_bar = self.mp.insert(0, ProgressBar::new(count as u64));
        desc_bar.set_style(
            ProgressStyle::default_bar()
                .template(reader::PROGRESSBAR_YELLOW)
                .unwrap(),
        );
        let file = File::open(&file_path)
            .map_err(|e| format!("Failed to open file {}: {e}", file_path_str))?;
        let decoder = DecodeReaderBytesBuilder::new().build(file);
        let reader = BufReader::new(decoder);

        let mut content = String::new();
        let mut adoc_image_dir = String::new();
        for (index, line) in reader.lines().enumerate() {
            match line {
                Ok(line) => {
                    // adocを解析するための簡単な置換
                    let line = line
                        .replace("=====", "#####")
                        .replace("====", "####")
                        .replace("===", "###")
                        .replace("==", "##");

                    if let Some(caps) = ADOC_IMAGESDIR_REGEX.captures(&line) {
                        adoc_image_dir = caps.get(1).map_or("", |m| m.as_str()).to_string() + "/";
                    }

                    if self.config.is_need {
                        let mut image_desc = String::new();
                        // MD_IMG_REGEXでキャプチャできる場合、その内容を文字列として取り出す
                        if let Some(caps) = MD_IMG_REGEX.captures(&line) {
                            image_desc =
                                self.description_image(caps, &file_path, target_dir, &content);
                            // println!("### Described image in Markdown: {}", image_desc); // デバッグ用に画像説明を出力
                            desc_bar.inc(1);
                            desc_bar.set_message(format!(
                                "Describing Image {}: {}",
                                file_path.file_name().unwrap_or_default().to_string_lossy(),
                                &paragraph_str
                            ));
                        }
                        if let Some(caps) = ADOC_IMG_REGEX.captures(&line) {
                            let image_dir = target_dir.join(&adoc_image_dir);
                            image_desc =
                                self.description_image(caps, &file_path, &image_dir, &content);
                            // println!("### Described image in AsciiDoc: {}", image_desc); // デバッグ用に画像説明を出力
                            desc_bar.inc(1);
                            desc_bar.set_message(format!(
                                "Describing Image {}: {}",
                                file_path.file_name().unwrap_or_default().to_string_lossy(),
                                &paragraph_str
                            ));
                        }
                        content.push_str(&image_desc);
                    }
                    content.push_str(&line);
                    content.push('\n');
                }
                Err(e) => eprintln!("Error at line {}: {}", index + 1, e),
            }
        }
        desc_bar.finish_and_clear();
        for cnt_line in split_text(&content) {
            // Markdownの見出しを抽出
            if let Some(caps) = HEADING_REGEX.captures(&cnt_line) {
                if let Some(m) = caps.get(1) {
                    paragraph_str = m.as_str().to_string();
                }
            }
            let mut docs = Vec::new();
            let doc = make_document(&file_path_str, &paragraph_str, &cnt_line)?;
            docs.push(doc);
            res.extend(docs);
        }
        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        args,
        reader::{ImageDescriptionConfig, Reader, file_list, markdown::MarkdownReader},
    };

    // #[tokio::test(flavor = "multi_thread")]
    #[test]
    fn test_markdown_reader() {
        println!("### Starting test for MarkdownReader..."); // デバッグ用にテスト開始を出力
        let args = args::Args {
            path: ".".to_string(),
            init: false,
            model: Some("qwen3.5".to_string()),
            url: Some("http://192.168.150.43:11434".to_string()),
            ..Default::default()
        };
        println!(
            "### Testing MarkdownReader with model: {} and URL: {}",
            &args.model.clone().unwrap(),
            &args.url.clone().unwrap()
        ); // デバッグ用にテスト情報を出力
        let config = ImageDescriptionConfig {
            is_need: true,
            model: args.model.clone().unwrap(),
            system: "You are a helpful assistant that describes the content of images in detail. Please provide a detailed description of the image, including any relevant information that can be inferred from the image. If the image contains text, please include the text in your description.".to_string(),
            temperature: args.temperature.unwrap_or(0.2) as f32,
            url: args.url.clone().unwrap(),
        };
        let mp = Arc::new(MultiProgress::new());
        let reader = MarkdownReader::new(config, Arc::clone(&mp));
        let target_dir = Path::new(".");
        let entry = file_list::EntryInfo {
            path: "../sample/doc-zenn/articles/10d085af9b48f2.md".to_string(),
            suffix: "md".to_string(),
            kind: "file",
            size: 0,
            modified: String::new(),
        };
        println!("### Testing MarkdownReader with file: {}", entry.path); // デバッグ用にテストファイルを出力
        let docs = reader.read(target_dir, &entry).unwrap();
        assert!(
            !docs.is_empty(),
            "Expected to extract documents from Markdown"
        );
    }
}

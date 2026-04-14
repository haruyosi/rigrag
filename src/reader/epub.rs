use std::{path::Path, sync::Arc};

use base64::{Engine, prelude::BASE64_STANDARD};
use epub::doc::EpubDoc;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use regex::Regex;
use scraper::{ElementRef, Html};

use crate::reader::{
    self, ChunkedDocument, ImageDescriptionConfig, Reader, describe_image, file_list,
    make_document, split_text,
};

lazy_static::lazy_static! {
    static ref TAG_REGEX: Regex = Regex::new(r"<[^>]*>").unwrap();
}

pub struct EpubReader {
    config: ImageDescriptionConfig,
    mp: Arc<MultiProgress>,
}

impl EpubReader {
    pub fn new(config: ImageDescriptionConfig, mp: Arc<MultiProgress>) -> Self {
        Self { config, mp }
    }

    fn split_push_doc(
        &self,
        paragraph: &str,
        fileinfo: &str,
        content: &str,
    ) -> Result<Vec<ChunkedDocument>, String> {
        split_text(content)
            .into_iter()
            .map(|cnt_line| make_document(fileinfo, paragraph, &cnt_line))
            .collect()
    }

    fn flatten_element<'b>(&self, vec: &mut Vec<ElementRef<'b>>, element_ref: ElementRef<'b>) {
        vec.push(element_ref);
        for child in element_ref.children() {
            if let Some(child_element) = ElementRef::wrap(child) {
                self.flatten_element(vec, child_element);
            }
        }
    }

    fn remove_tags(&self, html: &str) -> String {
        let re = &*TAG_REGEX;
        re.replace_all(html, "")
            .to_string()
            // remove full-width spaces
            .replace("\u{3000}", " ")
    }
}

impl Reader for EpubReader {
    fn read(
        &self,
        target_dir: &Path,
        entry: &file_list::EntryInfo,
    ) -> Result<Vec<ChunkedDocument>, String> {
        if !entry.suffix.eq_ignore_ascii_case("epub") {
            return Ok(Vec::new());
        }
        let mut res = Vec::new();

        let file_path = target_dir.join(&entry.path);
        let mut fileinfo = file_path.display().to_string();

        let mut epub_doc = EpubDoc::new(&file_path)
            .map_err(|e| format!("Failed to open EPUB file {}: {}", fileinfo, e))?;

        // for (k, v) in &epub_doc.resources {
        //     println!("Resource: {}, {:?}", k, v);
        // }
        // scan image number for later use (e.g. decide whether to describe images or not)
        let image_num = epub_doc
            .resources
            .iter()
            .filter(|(_, v)| v.mime.starts_with("image"))
            .count();
        // if there are many images, it may take a long time to describe them, so we can skip describing images in that case
        let desc_bar = self.mp.insert(0, ProgressBar::new(image_num as u64));
        desc_bar.set_style(
            ProgressStyle::default_bar()
                .template(reader::PROGRESSBAR_YELLOW)
                .unwrap(),
        );

        let mut paragraph = String::new();
        let mut content = String::new();

        for _ in 0..epub_doc.get_num_chapters() {
            let body = epub_doc
                .get_current_str()
                .map(|(content, _)| content)
                .unwrap_or_default();
            let html = Html::parse_document(&body);
            let document = html.root_element();
            let mut html_vec = Vec::new();
            self.flatten_element(&mut html_vec, document);

            // Find the tags containing the text to be analyzed.
            for tag in html_vec {
                match tag.value().name() {
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        if !content.is_empty() {
                            let docs = self.split_push_doc(&paragraph, &fileinfo, &content)?;
                            res.extend(docs);
                            content.clear();
                        }
                        paragraph = self.remove_tags(&tag.inner_html());
                        fileinfo = file_path.display().to_string();
                    }
                    "p" | "td" | "li" => {
                        content.push_str(&self.remove_tags(&tag.inner_html()));
                    }
                    "pre" => {
                        content.push_str(&self.remove_tags(&tag.inner_html()));
                    }
                    "img" => {
                        if self.config.is_need {
                            let src = tag.value().attr("src").unwrap_or_default();
                            let stem = Path::new(src)
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or_default();
                            let (image_bytes, _) = epub_doc.get_resource(stem).unwrap_or_default();
                            let image_base64 = BASE64_STANDARD.encode(&image_bytes);
                            let res = describe_image(&self.config, &image_base64, &content)?;
                            content.push_str(&res);
                            desc_bar.inc(1);
                            desc_bar.set_message(format!(
                                "Describing Image {}: {}",
                                file_path.file_name().unwrap_or_default().to_string_lossy(),
                                &paragraph
                            ));
                        }
                    }
                    _ => {}
                }
            }
            epub_doc.go_next();
        }
        desc_bar.finish_and_clear();
        let docs = self.split_push_doc(&paragraph, &fileinfo, &content)?;
        res.extend(docs);
        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use crate::args;

    use super::*;

    // #[tokio::test(flavor = "multi_thread")]
    #[test]
    fn test_epub_reader() {
        let args = args::Args {
            path: ".".to_string(),
            init: false,
            model: Some("qwen3.5".to_string()),
            url: Some("http://192.168.150.43:11434".to_string()),
            ..Default::default()
        };

        let config = ImageDescriptionConfig {
            is_need: args.picture.clone(),
            model: args.model.clone().unwrap(),
            system: "You are a helpful assistant that describes the content of images in detail. Please provide a detailed description of the image, including any relevant information that can be inferred from the image. If the image contains text, please include the text in your description.".to_string(),
            temperature: args.temperature.unwrap_or(0.2) as f32,
            url: args.url.clone().unwrap(),
        };
        let mp = Arc::new(MultiProgress::new());
        let reader = EpubReader::new(config, Arc::clone(&mp));
        let target_dir = Path::new(".");
        let entry = file_list::EntryInfo {
            path: "jouhoutsushin2025.epub".to_string(),
            suffix: "epub".to_string(),
            kind: "file",
            size: 0,
            modified: String::new(),
        };
        let docs = reader.read(target_dir, &entry).unwrap();
        assert!(docs.is_empty(), "Expected to extract documents from EPUB");
    }
}

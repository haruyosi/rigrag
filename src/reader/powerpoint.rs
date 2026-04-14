use base64::{Engine, prelude::BASE64_STANDARD};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rustypptx::ElementType;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::{path::Path, sync::Arc};

use crate::reader::{
    self, ChunkedDocument, ImageDescriptionConfig, Reader, describe_image, file_list,
    make_document, split_text,
};

pub struct PptxReader {
    config: ImageDescriptionConfig,
    mp: Arc<MultiProgress>,
}

impl PptxReader {
    pub fn new(config: ImageDescriptionConfig, mp: Arc<MultiProgress>) -> Self {
        Self { config, mp }
    }

    fn split_push_doc(
        &self,
        paragraph: &str,
        fileinfo: &str,
        content: &str,
    ) -> Result<Vec<ChunkedDocument>, String> {
        let mut res = Vec::new();
        for cnt_line in split_text(content) {
            let doc = make_document(fileinfo, paragraph, &cnt_line)?;
            res.push(doc);
        }
        Ok(res)
    }
}

impl Reader for PptxReader {
    fn read(
        &self,
        target_dir: &Path,
        entry: &file_list::EntryInfo,
    ) -> Result<Vec<ChunkedDocument>, String> {
        let mut res = Vec::new();
        if !entry.suffix.eq_ignore_ascii_case("pptx") {
            return Ok(Vec::new());
        }

        let file_path = target_dir.join(&entry.path);
        let fileinfo = file_path.display().to_string();
        let pptdoc = match catch_unwind(AssertUnwindSafe(|| rustypptx::parse_pptx(&file_path))) {
            Err(_) => {
                println!("Panic occurred while reading PPTX file: {fileinfo} (skipped)");
                return Ok(Vec::new());
            }
            Ok(Err(e)) => {
                println!("Can not read PPTX file: {fileinfo}: {e} (skipped)");
                return Ok(Vec::new());
            }
            Ok(Ok(doc)) => doc,
        };
        let mut fileinfo = file_path.display().to_string();
        let mut paragraph_str = String::new();
        let mut content = String::new();

        // scan image number for later use (e.g. decide whether to describe images or not)
        let image_num = pptdoc.slides.iter().map(|s| s.images.len()).sum::<usize>();
        // if there are many images, it may take a long time to describe them, so we can skip describing images in that case
        let desc_bar = self.mp.insert(0, ProgressBar::new(image_num as u64));
        desc_bar.set_style(
            ProgressStyle::default_bar()
                .template(reader::PROGRESSBAR_YELLOW)
                .unwrap(),
        );
        // scan text elements and describe images if exist
        for slide in pptdoc.slides {
            for element in slide.text_elements {
                match element.element_type {
                    ElementType::Title => {
                        if !content.is_empty() {
                            let docs = self.split_push_doc(&paragraph_str, &fileinfo, &content)?;
                            res.extend(docs);
                            content.clear();
                        }
                        paragraph_str = element.text.clone();
                        fileinfo = file_path.display().to_string();
                    }
                    ElementType::Subtitle => content.push_str(&element.text),
                    ElementType::Paragraph => content.push_str(&format!("\n{}", &element.text)),
                    ElementType::ListItem => content.push_str(&format!("\n- {}", &element.text)),
                }
            }
            if self.config.is_need {
                for image in slide.images {
                    let image_base64 = BASE64_STANDARD.encode(image.data);
                    let res = describe_image(&self.config, &image_base64, &content)?;
                    content.push_str(&res);
                    desc_bar.inc(1);
                    desc_bar.set_message(format!(
                        "Describing Image {}: {}",
                        file_path.file_name().unwrap_or_default().to_string_lossy(),
                        &paragraph_str
                    ));
                }
            }
        }
        desc_bar.finish_and_clear();
        let docs = self.split_push_doc(&paragraph_str, &fileinfo, &content)?;
        res.extend(docs);
        Ok(res)
    }
}

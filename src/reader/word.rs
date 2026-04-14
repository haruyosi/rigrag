use base64::{Engine, prelude::BASE64_STANDARD};
use docx_rs::{DocumentChild, ParagraphChild, RunChild};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::{fs::File, io::Read, path::Path};

use crate::reader::{
    self, ChunkedDocument, ImageDescriptionConfig, Reader, describe_image, file_list,
    make_document, split_text,
};

pub struct MsWordReader {
    config: ImageDescriptionConfig,
    mp: Arc<MultiProgress>,
}

impl MsWordReader {
    pub fn new(config: ImageDescriptionConfig, mp: Arc<MultiProgress>) -> Self {
        Self { config, mp }
    }

    fn sprit_push_doc(
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
impl Reader for MsWordReader {
    fn read(
        &self,
        target_dir: &Path,
        entry: &file_list::EntryInfo,
    ) -> Result<Vec<ChunkedDocument>, String> {
        let mut res = Vec::new();
        if !entry.suffix.eq_ignore_ascii_case("docx") {
            return Ok(Vec::new());
        }
        let file_path = target_dir.join(&entry.path);
        let mut fileinfo = file_path.display().to_string();
        let mut file =
            File::open(&file_path).map_err(|e| format!("Cannot open file {}: {e}", fileinfo))?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).unwrap();

        // Use catch_unwind to handle potential panics from docx_rs::read_docx
        let docx = match catch_unwind(AssertUnwindSafe(|| docx_rs::read_docx(&buf))) {
            Ok(Ok(text)) => text,
            Ok(Err(error)) => {
                println!("Failed to extract file from {}: {}", fileinfo, error);
                return Ok(Vec::new());
            }
            Err(_) => {
                println!("Panic occurred while extracting file from {}", fileinfo);
                return Ok(Vec::new());
            }
        };
        let mut paragraph_str = String::new();
        let mut content = String::new();
        for child in docx.document.children {
            match child {
                DocumentChild::Paragraph(p) => {
                    let is_paragraph = match p.property.style {
                        Some(s) => s.val.starts_with("Heading"),
                        None => false,
                    };
                    for run in p.children {
                        if let ParagraphChild::Run(r) = run {
                            for run_child in r.children {
                                if let RunChild::Text(t) = run_child {
                                    if is_paragraph {
                                        paragraph_str = t.text.clone();
                                        let docs = self.sprit_push_doc(
                                            &paragraph_str,
                                            &fileinfo,
                                            &content,
                                        )?;
                                        res.extend(docs);
                                        content.clear();
                                        fileinfo = file_path.display().to_string();
                                    } else {
                                        content.push_str(&t.text);
                                    }
                                }
                            }
                        }
                    }
                }
                DocumentChild::Table(_t) => {}
                _ => {}
            }
        }
        let docs = self.sprit_push_doc(&paragraph_str, &fileinfo, &content)?;
        res.extend(docs);

        if self.config.is_need {
            let desc_bar = self
                .mp
                .insert(0, ProgressBar::new(docx.images.len() as u64));
            desc_bar.set_style(
                ProgressStyle::default_bar()
                    .template(reader::PROGRESSBAR_YELLOW)
                    .unwrap(),
            );
            for (_, path, img, _) in docx.images {
                let image_base64 = BASE64_STANDARD.encode(img.0);
                let described_str = describe_image(
                    &self.config,
                    &image_base64,
                    "", // Empty context for now, as we don't have a clear way to associate text with images in DOCX. This can be improved in the future by analyzing the document structure more deeply.
                )?;
                desc_bar.inc(1);
                desc_bar.set_message(format!("Describing Image {}", &path));
                let docs = self.sprit_push_doc("", &fileinfo, &described_str)?;
                res.extend(docs);
            }
            desc_bar.finish_and_clear();
        }
        Ok(res)
    }
}

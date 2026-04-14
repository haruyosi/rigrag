use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::Arc;

use indicatif::MultiProgress;
use pdf_oxide::PdfDocument;
use pdf_oxide::converters::ConversionOptions;

use crate::reader::{
    ChunkedDocument, ImageDescriptionConfig, Reader, file_list, make_document, split_text,
};

pub struct PdfReader {
    config: ImageDescriptionConfig,
    _mp: Arc<MultiProgress>,
}

impl PdfReader {
    pub fn new(config: ImageDescriptionConfig, mp: Arc<MultiProgress>) -> Self {
        Self { config, _mp: mp }
    }

    fn _split_push_doc(
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
}

impl Reader for PdfReader {
    fn read(
        &self,
        target_dir: &Path,
        entry: &file_list::EntryInfo,
    ) -> Result<Vec<ChunkedDocument>, String> {
        if !entry.suffix.eq_ignore_ascii_case("pdf") {
            return Ok(Vec::new());
        }
        let mut res = Vec::new();

        let file_path = target_dir.join(&entry.path);
        let fileinfo = file_path.display().to_string();
        // Use catch_unwind to handle potential panics from pdf_extract
        let mut pdf_doc = match catch_unwind(AssertUnwindSafe(|| PdfDocument::open(&file_path))) {
            Ok(Ok(doc)) => doc,
            Ok(Err(error)) => {
                println!("Failed to open PDF file {}: {}", fileinfo, error);
                return Ok(Vec::new());
            }
            Err(_) => {
                println!("Panic occurred while opening PDF file {}", fileinfo);
                return Ok(Vec::new());
            }
        };
        let _need_images = self.config.is_need;
        // Disable aggressive features that may cause memory exhaustion
        let options = ConversionOptions {
            include_images: false, // Disable images to prevent memory explosion
            extract_tables: false, // Disable table extraction (can cause layout computation issues)
            detect_headings: true,
            ..ConversionOptions::default()
        };
        let page_count = pdf_doc
            .page_count()
            .map_err(|_| format!("Failed to get page count for PDF file {}", fileinfo))?;

        // Wrap page processing in catch_unwind to handle potential panics from pdf_oxide
        let extraction_result = catch_unwind(AssertUnwindSafe(|| {
            for page_index in 0..page_count {
                // Add page pre-check: skip pages with extremely large dimensions
                // (these can trigger memory exhaustion in pdf_oxide)
                match pdf_doc.get_page_media_box(page_index) {
                    Ok((x1, y1, x2, y2)) => {
                        let width = (x2 - x1).abs();
                        let height = (y2 - y1).abs();
                        let area = width * height;
                        // Skip pages with area > 10 million sq units (roughly A0 paper@300dpi)
                        if area > 10_000_000.0 {
                            println!(
                                "Warning: Page {} has extreme dimensions ({}x{}), skipping to avoid memory issues",
                                page_index, width, height
                            );
                            continue;
                        }
                    }
                    Err(e) => {
                        println!(
                            "Warning: Failed to get page mediabox for page {}: {}",
                            page_index, e
                        );
                        continue;
                    }
                }

                match pdf_doc.to_markdown(page_index, &options) {
                    Ok(md_content) => {
                        // Check content size sanity: skip if content is > 10MB (likely corrupted)
                        if md_content.len() > 10_000_000 {
                            println!(
                                "Warning: Page {} produced {} bytes of content (>10MB), likely corrupted, skipping",
                                page_index,
                                md_content.len()
                            );
                            continue;
                        }
                        // let content = content.trim();
                        let paragraph = format!("Page {}", page_index + 1);
                        let mut content = String::new();
                        for line in md_content.lines() {
                            if line.trim().is_empty() {
                                continue;
                            }
                            let line = line.replace(" ", "");
                            content.push_str(&line);
                            content.push('\n');
                        }

                        for cnt_line in split_text(&content) {
                            if let Ok(doc) = make_document(&fileinfo, &paragraph, &cnt_line) {
                                res.push(doc);
                            }
                        }
                    }
                    Err(e) => {
                        println!(
                            "Warning: Failed to extract text from page {} of {}: {}",
                            page_index, fileinfo, e
                        );
                        // Continue with next page instead of returning error
                        continue;
                    }
                }
            }
        }));

        match extraction_result {
            Ok(()) => {
                if res.is_empty() {
                    // println!("Warning: No content extracted from PDF {}", fileinfo);
                }
                Ok(res)
            }
            Err(_) => {
                println!("Panic occurred during page processing for PDF {}", fileinfo);
                // Return whatever was extracted before the panic
                if res.is_empty() {
                    Ok(Vec::new())
                } else {
                    Ok(res)
                }
            }
        }
    }
}

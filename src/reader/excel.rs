use base64::{Engine, prelude::BASE64_STANDARD};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::Arc;
use umya_spreadsheet::{Spreadsheet, Worksheet, reader::xlsx};

use crate::reader::{
    self, ChunkedDocument, ImageDescriptionConfig, Reader, describe_image, file_list,
    make_document, split_text,
};

pub struct ExcelReader {
    config: ImageDescriptionConfig,
    mp: Arc<MultiProgress>,
}

impl ExcelReader {
    pub fn new(config: ImageDescriptionConfig, mp: Arc<MultiProgress>) -> Self {
        Self { config, mp }
    }

    /// read file content (xlsx format)
    fn read_xlsx(
        &self,
        file_path: &Path,
        workbook: Spreadsheet,
    ) -> Result<Vec<ChunkedDocument>, String> {
        let mut res = Vec::new();
        for sheet in workbook.get_sheet_collection() {
            let sheet_name = sheet.get_name();
            let docs = self.read_range(file_path, &sheet_name, &sheet)?;
            res.extend(docs);
        }
        Ok(res)
    }

    /// read cell content and describe images in the sheet if exists
    fn read_range(
        &self,
        file_path: &Path,
        sheet_name: &str,
        sheet: &Worksheet,
    ) -> Result<Vec<ChunkedDocument>, String> {
        let mut content = String::new();
        let mut res = Vec::new();
        // read stings in cells and concatenate them into content variable
        for cell in sheet.get_cell_collection_sorted() {
            let val = cell.get_value();
            if val.is_empty() {
                continue;
            }
            content.push_str(&val);
            content.push(' ');
        }
        content.push('\n');
        if self.config.is_need {
            let count = sheet.get_image_collection().len();
            let desc_bar = self.mp.insert(0, ProgressBar::new(count as u64));
            desc_bar.set_style(
                ProgressStyle::default_bar()
                    .template(reader::PROGRESSBAR_YELLOW)
                    .unwrap(),
            );
            // read image information
            for image in sheet.get_image_collection() {
                let image_base64 = BASE64_STANDARD.encode(image.get_image_data());
                let res =
                    describe_image(&self.config, &image_base64, &content).unwrap_or(String::new());
                content.push_str(&res);
                desc_bar.inc(1);
                desc_bar.set_message(format!(
                    "Describing Image {}: {}",
                    file_path.file_name().unwrap_or_default().to_string_lossy(),
                    sheet_name
                ));
            }
            desc_bar.finish_and_clear();
        }
        let fileinfo = file_path.display().to_string();
        for cnt_line in split_text(&content) {
            let doc = make_document(&fileinfo, sheet_name, &cnt_line)?;
            res.push(doc);
        }
        Ok(res)
    }
}

impl Reader for ExcelReader {
    fn read(
        &self,
        target_dir: &Path,
        entry: &file_list::EntryInfo,
    ) -> Result<Vec<ChunkedDocument>, String> {
        let suffix_lower = entry.suffix.to_lowercase();

        if !matches!(suffix_lower.as_str(), "xlsx") {
            return Ok(Vec::new());
        }

        let file_path = target_dir.join(&entry.path);
        let fileinfo = file_path.display().to_string();

        // Use catch_unwind to handle potential panics from xlsx::read
        let workbook = match catch_unwind(AssertUnwindSafe(|| xlsx::read(&file_path))) {
            Ok(Ok(wb)) => wb,
            Ok(Err(error)) => {
                println!("Failed to extract Excel file from {}: {}", &fileinfo, error);
                return Ok(Vec::new());
            }
            Err(_) => {
                println!(
                    "Panic occurred while extracting Excel file from {}",
                    &fileinfo
                );
                return Ok(Vec::new());
            }
        };
        self.read_xlsx(&file_path, workbook)
    }
}

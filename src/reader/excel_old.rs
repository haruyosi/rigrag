use calamine::{self, Data, Range, Reader};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::{fs::File, io::BufReader, path::Path};

use crate::reader::{self, ChunkedDocument, file_list, make_document, split_text};

#[derive(Default)]
pub struct ExcelOldReader {}

impl ExcelOldReader {
    pub fn new() -> Self {
        Self::default()
    }

    /// Read the content of the file (xls format)
    fn read_xls(
        &self,
        file_path: &Path,
        mut workbook: calamine::Xls<BufReader<File>>,
    ) -> Result<Vec<ChunkedDocument>, String> {
        let mut res = Vec::new();
        for sheet_name in workbook.sheet_names() {
            let range = workbook
                .worksheet_range(&sheet_name)
                .map_err(|e| format!("Can not read sheet '{}' : {}", sheet_name, e))?;
            let docs = self.read_range(file_path, &sheet_name, &range)?;
            res.extend(docs);
        }
        Ok(res)
    }

    /// Read cells from the given range and create documents for each line of text.
    fn read_range(
        &self,
        file_path: &Path,
        sheet_name: &str,
        range: &Range<Data>,
    ) -> Result<Vec<ChunkedDocument>, String> {
        let mut content = String::new();
        let mut res = Vec::new();
        for row in range.rows() {
            for cell in row {
                content.push_str(&cell.to_string());
                content.push(' ');
            }
            content.push('\n');
        }
        let fileinfo = file_path.display().to_string();
        for cnt_line in split_text(&content) {
            let doc = make_document(&fileinfo, sheet_name, &cnt_line)?;
            res.push(doc);
        }
        Ok(res)
    }
}

impl reader::Reader for ExcelOldReader {
    fn read(
        &self,
        target_dir: &Path,
        entry: &file_list::EntryInfo,
    ) -> Result<Vec<ChunkedDocument>, String> {
        let suffix_lower = entry.suffix.to_lowercase();
        if !matches!(suffix_lower.as_str(), "xls") {
            return Ok(Vec::new());
        }
        let file_path = target_dir.join(&entry.path);
        let fileinfo = file_path.display().to_string();

        // Use catch_unwind to handle potential panics from calamine::open_workbook
        let workbook: calamine::Xls<BufReader<File>> =
            match catch_unwind(AssertUnwindSafe(|| calamine::open_workbook(&file_path))) {
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
        self.read_xls(&file_path, workbook)
    }
}

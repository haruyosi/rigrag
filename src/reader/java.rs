use rust_code_analysis::JavaParser;
use rust_code_analysis::ParserTrait;
use rust_code_analysis::function;
use std::fs;
use std::path::Path;

use crate::reader::{ChunkedDocument, Reader, file_list, make_document, split_code};

pub struct JavaReader {}

impl JavaReader {
    pub fn new() -> Self {
        Self {}
    }

    fn method_snippet(&self, code_lines: &[&str], start_line: usize, end_line: usize) -> String {
        if start_line == 0 || start_line > end_line {
            return String::new();
        }

        let start_idx = start_line.saturating_sub(1);
        let end_idx = end_line.min(code_lines.len());
        if start_idx >= end_idx {
            return String::new();
        }

        let snippet_lines = &code_lines[start_idx..end_idx];
        let capacity = snippet_lines.iter().map(|line| line.len() + 1).sum();
        let mut snippet = String::with_capacity(capacity);

        for (idx, line) in snippet_lines.iter().enumerate() {
            if idx > 0 {
                snippet.push('\n');
            }

            // Remove leading whitespace from each line in the snippet.
            // snippet.push_str(line.trim_start());
            snippet.push_str(line);
        }

        snippet
    }

    fn extract_javadoc(&self, code_lines: &[&str], start_line: usize) -> Option<String> {
        if start_line <= 1 {
            return None;
        }

        let mut idx = start_line.saturating_sub(2);
        loop {
            let trimmed = code_lines.get(idx)?.trim();
            if trimmed.is_empty() || trimmed.starts_with('@') {
                if idx == 0 {
                    return None;
                }
                idx -= 1;
                continue;
            }
            break;
        }

        if !code_lines.get(idx)?.trim_end().ends_with("*/") {
            return None;
        }

        let end_idx = idx;
        loop {
            let trimmed = code_lines.get(idx)?.trim_start();
            if trimmed.starts_with("/**") {
                let raw_lines = &code_lines[idx..=end_idx];
                let cleaned = raw_lines
                    .iter()
                    .map(|line| {
                        line.trim()
                            .trim_start_matches("/**")
                            .trim_start_matches("/*")
                            .trim_start_matches('*')
                            .trim_end_matches("*/")
                            .trim()
                            .to_string()
                    })
                    .filter(|line| !line.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n");

                return (!cleaned.is_empty()).then_some(cleaned);
            }

            if idx == 0 {
                return None;
            }
            idx -= 1;
        }
    }

    fn split_push_doc(
        &self,
        paragraph: &str,
        fileinfo: &str,
        content: &str,
    ) -> Result<Vec<ChunkedDocument>, String> {
        let mut res = Vec::new();
        for cnt_line in split_code(&content) {
            let doc = make_document(fileinfo, paragraph, &cnt_line)?;
            // println!("----------------------\n{}", doc.doc);
            res.push(doc);
        }
        Ok(res)
    }
}

impl Reader for JavaReader {
    fn read(
        &self,
        target_dir: &Path,
        entry: &file_list::EntryInfo,
    ) -> Result<Vec<ChunkedDocument>, String> {
        if !entry.suffix.eq_ignore_ascii_case("java") {
            return Ok(Vec::new());
        }

        let file_path = target_dir.join(&entry.path);
        let fileinfo = file_path.display().to_string();
        let code = fs::read_to_string(&file_path)
            .map_err(|e| format!("Failed to read file {}: {e}", fileinfo))?;
        let code_lines = code.lines().collect::<Vec<_>>();
        let parser = JavaParser::new(code.as_bytes().to_vec(), &file_path, None);
        let mut docs = Vec::new();

        for span in function(&parser) {
            if span.error {
                continue;
            }

            let snippet = self.method_snippet(&code_lines, span.start_line, span.end_line);
            let javadoc = self.extract_javadoc(&code_lines, span.start_line);
            let content = match javadoc {
                Some(doc) => {
                    let mut content = String::with_capacity(doc.len() + snippet.len() + 1);
                    content.push_str(&doc);
                    content.push('\n');
                    content.push_str(&snippet);
                    content
                }
                None => snippet,
            };
            docs.extend(self.split_push_doc(&span.name, &fileinfo, &content)?);
        }
        Ok(docs)
    }
}

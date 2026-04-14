use crate::reader::{ChunkedDocument, Reader, file_list, make_document, split_text};
use encoding_rs::{EUC_JP, Encoding, ISO_2022_JP, SHIFT_JIS, UTF_16BE, UTF_16LE, UTF_8, WINDOWS_1252};
use std::{
    fs::File,
    io::Read,
    path::Path,
};

pub struct TextReader {}

impl TextReader {
    pub fn new() -> Self {
        Self {}
    }
}

impl Reader for TextReader {
    fn read(
        &self,
        target_dir: &Path,
        entry: &file_list::EntryInfo,
    ) -> Result<Vec<ChunkedDocument>, String> {
        let suffixs = vec!["txt"];
        let mut res = Vec::new();
        if !suffixs.contains(&entry.suffix.to_lowercase().as_str()) {
            return Ok(res);
        }
        let file_path = target_dir.join(&entry.path);
        let file_path_str = file_path
            .to_str()
            .ok_or_else(|| "Invalid file path".to_string())?
            .to_string();

        let file = File::open(&file_path)
            .map_err(|e| format!("Failed to open file {}: {e}", file_path_str))?;
        let mut bytes = Vec::new();
        let mut file_reader = file;
        file_reader
            .read_to_end(&mut bytes)
            .map_err(|e| format!("Failed to read file {}: {e}", file_path_str))?;

        let content = decode_text_with_fallback(&bytes);
        for cnt_line in split_text(&content) {
            let mut docs = Vec::new();
            let doc = make_document(&file_path_str, "", &cnt_line)?;
            docs.push(doc);
            res.extend(docs);
        }
        Ok(res)
    }
}

fn decode_text_with_fallback(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }

    if let Some((encoding, bom_len)) = Encoding::for_bom(bytes) {
        let (decoded, _, _) = encoding.decode(&bytes[bom_len..]);
        return decoded.into_owned();
    }

    let candidates = [UTF_8, SHIFT_JIS, EUC_JP, ISO_2022_JP, UTF_16LE, UTF_16BE, WINDOWS_1252];

    let mut best_text = String::new();
    let mut best_score = usize::MAX;
    for encoding in candidates {
        let (decoded, _, _) = encoding.decode(bytes);
        let decoded_text = decoded.into_owned();
        let replacement_count = decoded_text.matches('\u{FFFD}').count();

        if replacement_count < best_score {
            best_score = replacement_count;
            best_text = decoded_text;
            if best_score == 0 {
                break;
            }
        }
    }

    best_text
}

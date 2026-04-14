use anyhow::Result;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use lancedb::Connection;
use rig::OneOrMany;
use rig::embeddings::{Embedding, EmbeddingsBuilder};
use rig::providers::ollama::EmbeddingModel;
use rig::vector_store::InsertDocuments;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::conf::AppConfig;
use crate::db_store::LanceDbDocumentStore;
use crate::reader::text::TextReader;
use crate::reader::{
    self, ChunkedDocument, Reader, epub::EpubReader, excel::ExcelReader, excel_old::ExcelOldReader,
    java::JavaReader, markdown::MarkdownReader, pdf::PdfReader, powerpoint::PptxReader,
    word::MsWordReader,
};

const EXTENSIONS: &[&str] = &[
    "md", "adoc", "xlsx", "xls", "docx", "pptx", "pdf", "epub", "txt", "java",
];

const EMBED_MAX_CHARS: usize = 1800;
const EMBED_OVERLAP_CHARS: usize = 200;
const EMBED_MIN_CHARS: usize = 100;

fn split_document_for_embedding(
    doc: &ChunkedDocument,
    max_chars: usize,
    overlap_chars: usize,
) -> Vec<ChunkedDocument> {
    let chars: Vec<char> = doc.doc.chars().collect();
    if chars.len() <= max_chars {
        return vec![doc.clone()];
    }

    let mut chunks = Vec::new();
    let step = max_chars.saturating_sub(overlap_chars).max(1);
    let mut start = 0usize;
    let mut part = 1usize;

    while start < chars.len() {
        let end = (start + max_chars).min(chars.len());
        let chunk_text: String = chars[start..end].iter().collect();
        chunks.push(ChunkedDocument {
            path: doc.path.clone(),
            chapter: format!("{} (part {})", doc.chapter, part),
            doc: chunk_text,
        });

        if end == chars.len() {
            break;
        }

        start += step;
        part += 1;
    }

    chunks
}

fn normalize_documents_for_embedding(docs: &[ChunkedDocument]) -> Vec<ChunkedDocument> {
    docs.iter()
        .flat_map(|doc| split_document_for_embedding(doc, EMBED_MAX_CHARS, EMBED_OVERLAP_CHARS))
        .collect()
}

fn log_context_length_candidates(
    docs: &[ChunkedDocument],
    label: &str,
    top_n: usize,
    verbose: bool,
) {
    if !verbose {
        return;
    }

    let mut candidates = docs
        .iter()
        .map(|doc| {
            (
                doc.path.as_str(),
                doc.chapter.as_str(),
                doc.doc.chars().count(),
            )
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|a, b| b.2.cmp(&a.2));

    eprintln!(
        "Warning: context length candidate docs in {} (top {}):",
        label,
        top_n.min(candidates.len())
    );
    for (idx, (path, chapter, char_len)) in candidates.into_iter().take(top_n).enumerate() {
        eprintln!(
            "  {}. chars={} path={} chapter={}",
            idx + 1,
            char_len,
            path,
            chapter
        );
    }
}

async fn try_embed_with_adaptive_sizing(
    embed_model: &EmbeddingModel,
    docs: &[ChunkedDocument],
    initial_max_chars: usize,
    verbose: bool,
) -> Result<Vec<(ChunkedDocument, OneOrMany<Embedding>)>, anyhow::Error> {
    let mut max_chars = initial_max_chars;

    loop {
        if max_chars < EMBED_MIN_CHARS {
            return Err(anyhow::anyhow!(
                "Failed to embed batch: chunk size reduced below minimum ({})",
                EMBED_MIN_CHARS
            ));
        }

        let overlap = (max_chars / 4).max(10);
        let retry_docs = docs
            .iter()
            .flat_map(|doc| split_document_for_embedding(doc, max_chars, overlap))
            .collect::<Vec<_>>();

        match EmbeddingsBuilder::new(embed_model.clone())
            .documents(retry_docs.clone())?
            .build()
            .await
        {
            Ok(embeddings) => {
                if verbose {
                    eprintln!("✓ Batch embedded successfully with max_chars={}", max_chars);
                }
                return Ok(embeddings);
            }
            Err(e) => {
                let err_msg = e.to_string();
                if err_msg.contains("input length exceeds the context length") {
                    eprintln!(
                        "Warning: Batch still exceeds context length at max_chars={}. Reducing to {}...",
                        max_chars,
                        max_chars / 2
                    );
                    log_context_length_candidates(
                        &retry_docs,
                        &format!("batch retry with max_chars={}", max_chars),
                        3,
                        verbose,
                    );
                    max_chars /= 2;
                } else {
                    return Err(e.into());
                }
            }
        }
    }
}

pub async fn crawl(
    config: &AppConfig,
    target_dir: PathBuf,
    embed_model: EmbeddingModel,
    db: &Connection,
) -> Result<()> {
    let entries = reader::file_list::scan_entries(&target_dir);
    let queue = Arc::new(Mutex::new(VecDeque::new()));
    let emb_queue = Arc::new(Mutex::new(VecDeque::new()));

    if config.chat.verbose {
        reader::file_list::_print_report(&target_dir, &entries);
    }

    for entry in entries {
        let suffix = entry.suffix.to_lowercase();
        if EXTENSIONS.contains(&suffix.as_str()) {
            if config.exclude.contains(&suffix) {
                continue;
            }
            queue.lock().unwrap().push_back(entry);
        }
    }

    let total_files = queue.lock().unwrap().len();
    println!("Total files to process: {}", total_files);
    let mp = Arc::new(MultiProgress::new());
    let prime_bar = Arc::new(mp.add(ProgressBar::new(total_files as u64)));
    prime_bar.set_style(
        ProgressStyle::default_bar()
            .template(reader::PROGRESSBAR_BLUE)
            .unwrap(),
    );
    let mut handles = Vec::new();
    let mut emb_handles = vec![];
    let thread_num = if config.single_thread {
        1
    } else {
        num_cpus::get()
    };
    let active_readers = Arc::new(AtomicUsize::new(thread_num));
    for i in 0..thread_num {
        let queue = Arc::clone(&queue);
        let bar = Arc::clone(&prime_bar);
        let target_dir = target_dir.clone();
        let emb_queue = Arc::clone(&emb_queue);
        let config = config.clone();
        let mp = Arc::clone(&mp);
        let active_readers = Arc::clone(&active_readers);
        let handle = thread::spawn(move || {
            bar.tick();
            let reader_map = create_reader_inst(&config, mp);
            loop {
                let t_entry = {
                    let mut guard = queue.lock().unwrap();
                    guard.pop_back()
                };
                let Some(t_entry) = t_entry else {
                    break;
                };
                bar.inc(1);
                bar.set_message(format!(
                    "{}",
                    Path::new(&t_entry.path)
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                ));
                let docs = reader_map
                    .get(&t_entry.suffix.to_lowercase())
                    .unwrap()
                    .read(&target_dir, &t_entry)
                    .unwrap_or_else(|e| {
                        panic!(
                            "Failed to read file {:?} in thread {}: {}",
                            t_entry.path, i, e
                        )
                    });
                for doc in docs {
                    emb_queue.lock().unwrap().push_back(doc);
                }
            }
            active_readers.fetch_sub(1, Ordering::SeqCst);
        });
        handles.push(handle);
    }
    println!("start embedding with {} threads...", config.embed_thread);
    // Vectorization using EmbeddingsBuilder without batching
    let embed_bar = Arc::new(mp.add(ProgressBar::new(0)));
    embed_bar.set_style(
        ProgressStyle::default_bar()
            .template(reader::PROGRESSBAR_GREEN)
            .unwrap(),
    );
    let embeddings = Arc::new(Mutex::new(Vec::new()));
    let emb_chunk_size = 64;
    for _i in 0..config.embed_thread {
        let emb_queue = Arc::clone(&emb_queue);
        let embed_bar = embed_bar.clone();
        let embed_model = embed_model.clone();
        let config = config.clone();
        let embeddings = Arc::clone(&embeddings);
        let active_readers = Arc::clone(&active_readers);
        let emb_handle = tokio::spawn(async move {
            loop {
                // Drop the lock as soon as possible to allow reader threads to continue pushing documents
                let chunks = {
                    let mut unlock_queue = emb_queue.lock().unwrap();
                    embed_bar.set_length(unlock_queue.len() as u64 + embed_bar.position());
                    if unlock_queue.is_empty() {
                        drop(unlock_queue);
                        None
                    } else {
                        let take_len = unlock_queue.len().min(emb_chunk_size);
                        let mut chunks: Vec<ChunkedDocument> = Vec::with_capacity(take_len);
                        for _ in 0..take_len {
                            if let Some(doc) = unlock_queue.pop_front() {
                                chunks.push(doc);
                            }
                        }
                        Some(chunks)
                    }
                };

                if let Some(chunks) = chunks {
                    embed_bar.inc(chunks.len() as u64);
                    embed_bar.set_message(format!(
                        "Vectorizing... remaining {}",
                        emb_queue.lock().unwrap().len()
                    ));
                    let verbose = config.chat.verbose;
                    let normalized_docs = normalize_documents_for_embedding(&chunks);
                    let batch_embeddings = match EmbeddingsBuilder::new(embed_model.clone())
                        .documents(normalized_docs.clone())
                    {
                        Ok(builder) => match builder.build().await {
                            Ok(emb) => emb,
                            Err(e) => {
                                let err_message = e.to_string();
                                if err_message.contains("input length exceeds the context length") {
                                    eprintln!(
                                        "Warning: context length exceeded. Retrying with smaller chunks."
                                    );
                                    log_context_length_candidates(
                                        &normalized_docs,
                                        &"batch initial attempt".to_string(),
                                        5,
                                        verbose,
                                    );
                                    try_embed_with_adaptive_sizing(
                                        &embed_model,
                                        &normalized_docs,
                                        900,
                                        verbose,
                                    )
                                    .await
                                    .unwrap_or_default()
                                } else {
                                    continue;
                                }
                            }
                        },
                        Err(_) => continue,
                    };
                    let mut emb_lock = embeddings.lock().unwrap();
                    emb_lock.extend(batch_embeddings);
                } else {
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    if active_readers.load(Ordering::SeqCst) == 0
                        && emb_queue.lock().unwrap().is_empty()
                    {
                        break;
                    }
                }
            }
        });
        emb_handles.push(emb_handle);
    }
    // Reader thread joins are blocking, so we wait for them in a separate thread to avoid blocking the async runtime
    tokio::task::spawn_blocking(move || {
        for handle in handles {
            handle.join().expect("Thread panicked");
        }
    })
    .await
    .map_err(|e| anyhow::anyhow!("Reader join task failed: {}", e))?;
    for handle in emb_handles {
        let _ = handle.await;
    }
    prime_bar.finish_and_clear();
    mp.clear()?;
    embed_bar.finish_and_clear();
    let embeddings = Arc::try_unwrap(embeddings).unwrap().into_inner().unwrap();

    println!("embeddings len is {}", embeddings.len());
    let dim = embeddings
        .first()
        .map(|(_, v)| v.first().vec.len())
        .unwrap_or(0);
    // Create or open the LanceDB table and insert documents
    let mut table_name = String::from("documentTbl_");
    table_name.push_str(&config.embed_model);
    let table;
    if LanceDbDocumentStore::is_exsist_table(&db, &table_name).await? {
        table = LanceDbDocumentStore::open_table(&db, &table_name).await?;
        table.delete("true").await?;
    } else {
        table = LanceDbDocumentStore::create_table(&db, &table_name, dim).await?;
    }
    let document_store = LanceDbDocumentStore::new(table.clone());
    document_store.insert_documents(embeddings).await?;
    println!("Documents inserted into DB");
    Ok(())
}

fn create_reader_inst(
    app_config: &AppConfig,
    mp: Arc<MultiProgress>,
) -> HashMap<String, Box<dyn Reader>> {
    let config = app_config.image.clone();
    let reader_map: HashMap<String, Box<dyn Reader>> = EXTENSIONS
        .iter()
        .map(|ext| {
            (
                ext.to_string(),
                match *ext {
                    "txt" => Box::new(TextReader::new()) as Box<dyn Reader>,
                    "docx" => Box::new(MsWordReader::new(config.clone(), Arc::clone(&mp)))
                        as Box<dyn Reader>,
                    "md" | "adoc" => Box::new(MarkdownReader::new(config.clone(), Arc::clone(&mp)))
                        as Box<dyn Reader>,
                    "pdf" => {
                        Box::new(PdfReader::new(config.clone(), Arc::clone(&mp))) as Box<dyn Reader>
                    }
                    "java" => Box::new(JavaReader::new()) as Box<dyn Reader>,
                    "xlsx" => Box::new(ExcelReader::new(config.clone(), Arc::clone(&mp)))
                        as Box<dyn Reader>,
                    "xls" => Box::new(ExcelOldReader::new()) as Box<dyn Reader>,
                    "pptx" => Box::new(PptxReader::new(config.clone(), Arc::clone(&mp)))
                        as Box<dyn Reader>,
                    "epub" => Box::new(EpubReader::new(config.clone(), Arc::clone(&mp)))
                        as Box<dyn Reader>,
                    _ => panic!("Unsupported file extension: {}", ext),
                },
            )
        })
        .collect();
    reader_map
}

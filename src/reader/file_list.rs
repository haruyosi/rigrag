use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

#[derive(Clone, Debug)]
pub struct EntryInfo {
    pub path: String,
    pub suffix: String,
    pub kind: &'static str,
    #[allow(dead_code)]
    pub size: u64,
    #[allow(dead_code)]
    pub modified: String,
}

pub fn scan_entries(target_dir: &Path) -> Vec<EntryInfo> {
    let entries = match collect_entries(&target_dir) {
        Ok(entries) => entries,
        Err(error) => {
            eprintln!("Failed to read directory {}: {error}", target_dir.display());
            std::process::exit(1);
        }
    };
    entries
}

fn collect_entries(dir: &Path) -> Result<Vec<EntryInfo>, std::io::Error> {
    let mut entries = Vec::new();
    collect_entries_recursive(dir, dir, &mut entries)?;
    // Delete directory entries
    entries.retain(|entry| entry.kind != "dir");
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(entries)
}

fn collect_entries_recursive(
    root: &Path,
    current: &Path,
    entries: &mut Vec<EntryInfo>,
) -> Result<(), std::io::Error> {
    for entry_result in fs::read_dir(current)? {
        let entry = entry_result?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        let metadata = entry.metadata()?;
        let kind = if file_type.is_file() {
            "file"
        } else if file_type.is_dir() {
            "dir"
        } else if file_type.is_symlink() {
            "symlink"
        } else {
            "other"
        };

        let size = if metadata.is_file() {
            metadata.len()
        } else {
            0
        };

        // Skip files and directories that start with a dot
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }

        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let relative_path = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .display()
            .to_string();

        let suffix: String = match path.extension() {
            Some(ext) => ext.to_string_lossy().to_string(),
            None => "none".to_string(),
        };

        entries.push(EntryInfo {
            path: relative_path,
            suffix,
            kind,
            size,
            modified,
        });

        if file_type.is_dir() {
            collect_entries_recursive(root, &path, entries)?;
        }
    }

    Ok(())
}

pub fn _print_report(dir: &Path, entries: &[EntryInfo]) {
    println!("Target directory: {}", dir.display());
    println!(
        "{:<60} {:<5} {:<10} {:>12} {:>15}",
        "path", "ext", "type", "size", "modified(unix)"
    );
    println!("{}", "-".repeat(100));

    let mut file_count = 0usize;
    let mut dir_count = 0usize;
    let mut total_size = 0u64;

    for entry in entries {
        match entry.kind {
            "file" => {
                file_count += 1;
                total_size += entry.size;
            }
            "dir" => dir_count += 1,
            _ => {}
        }

        println!(
            "{:<60} {:<5} {:<10} {:>12} {:>15}",
            entry.path, entry.suffix, entry.kind, entry.size, entry.modified
        );
    }

    println!("{}", "-".repeat(128));
    println!("File count: {file_count}");
    println!("Directory count: {dir_count}");
    println!("Total size: {total_size} bytes");
}

//! Incremental translation cache — tracks SHA-256 hashes of Java source files
//! so unchanged files can be skipped on subsequent runs.

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const CACHE_FILE: &str = ".jtrans-cache";

/// Maps canonical file paths to their last-known SHA-256 hex digest.
pub struct TranslationCache {
    entries: HashMap<PathBuf, String>,
}

impl TranslationCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Load the cache from `<output_dir>/.jtrans-cache`, or return an empty
    /// cache if the file does not exist or is malformed.
    pub fn load(output_dir: &Path) -> Self {
        let path = output_dir.join(CACHE_FILE);
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Self::new(),
        };

        let mut entries = HashMap::new();
        for line in content.lines() {
            if let Some((hash, file_path)) = line.split_once(' ') {
                entries.insert(PathBuf::from(file_path), hash.to_owned());
            }
        }
        Self { entries }
    }

    /// Save the cache to `<output_dir>/.jtrans-cache`.
    pub fn save(&self, output_dir: &Path) -> anyhow::Result<()> {
        std::fs::create_dir_all(output_dir)?;
        let path = output_dir.join(CACHE_FILE);
        let mut content = String::new();
        for (file_path, hash) in &self.entries {
            content.push_str(hash);
            content.push(' ');
            content.push_str(&file_path.to_string_lossy());
            content.push('\n');
        }
        std::fs::write(&path, &content)?;
        Ok(())
    }

    /// Returns `true` if the file has changed since the last recorded hash
    /// (or if it has never been seen before).
    pub fn is_stale(&self, java_file: &Path) -> anyhow::Result<bool> {
        let canonical = match std::fs::canonicalize(java_file) {
            Ok(p) => p,
            Err(_) => return Ok(true),
        };

        let current_hash = hash_file(&canonical)?;

        match self.entries.get(&canonical) {
            Some(cached_hash) if *cached_hash == current_hash => Ok(false),
            _ => Ok(true),
        }
    }

    /// Record the current hash for a file.
    pub fn update(&mut self, java_file: &Path) -> anyhow::Result<()> {
        let canonical = std::fs::canonicalize(java_file)?;
        let hash = hash_file(&canonical)?;
        self.entries.insert(canonical, hash);
        Ok(())
    }
}

fn hash_file(path: &Path) -> anyhow::Result<String> {
    let content = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&content);
    let digest = hasher.finalize();
    Ok(format!("{digest:x}"))
}

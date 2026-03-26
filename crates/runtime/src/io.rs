#![allow(non_snake_case)]
//! [`JFile`] — Rust representation of `java.io.File`.
//!
//! Mapping: `java.io.File` → `JFile` (wraps `std::path::PathBuf`).

use crate::string::JString;
use std::path::PathBuf;

/// Java `java.io.File` — an abstract path name.
#[derive(Debug, Clone, Default)]
pub struct JFile {
    path: PathBuf,
}

impl JFile {
    /// Java `new File(path)`.
    pub fn new(path: JString) -> Self {
        Self {
            path: PathBuf::from(path.as_str()),
        }
    }

    /// Java `new File(parent, child)`.
    pub fn new_child(parent: JString, child: JString) -> Self {
        let mut p = PathBuf::from(parent.as_str());
        p.push(child.as_str());
        Self { path: p }
    }

    /// Java `file.getName()`.
    pub fn getName(&self) -> JString {
        JString::from(self.path.file_name().and_then(|n| n.to_str()).unwrap_or(""))
    }

    /// Java `file.getPath()`.
    pub fn getPath(&self) -> JString {
        JString::from(self.path.to_str().unwrap_or(""))
    }

    /// Java `file.getAbsolutePath()`.
    pub fn getAbsolutePath(&self) -> JString {
        JString::from(
            std::fs::canonicalize(&self.path)
                .unwrap_or_else(|_| self.path.clone())
                .to_str()
                .unwrap_or(""),
        )
    }

    /// Java `file.getParent()`.
    pub fn getParent(&self) -> JString {
        JString::from(self.path.parent().and_then(|p| p.to_str()).unwrap_or(""))
    }

    /// Java `file.exists()`.
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// Java `file.isFile()`.
    pub fn isFile(&self) -> bool {
        self.path.is_file()
    }

    /// Java `file.isDirectory()`.
    pub fn isDirectory(&self) -> bool {
        self.path.is_dir()
    }

    /// Java `file.length()`.
    pub fn length(&self) -> i64 {
        std::fs::metadata(&self.path)
            .map(|m| m.len() as i64)
            .unwrap_or(0)
    }

    /// Java `file.delete()`.
    pub fn delete(&self) -> bool {
        if self.path.is_dir() {
            std::fs::remove_dir(&self.path).is_ok()
        } else {
            std::fs::remove_file(&self.path).is_ok()
        }
    }

    /// Java `file.mkdir()`.
    pub fn mkdir(&self) -> bool {
        std::fs::create_dir(&self.path).is_ok()
    }

    /// Java `file.mkdirs()`.
    pub fn mkdirs(&self) -> bool {
        std::fs::create_dir_all(&self.path).is_ok()
    }

    /// Java `file.toString()` / `file.getPath()`.
    pub fn toString(&self) -> JString {
        self.getPath()
    }
}

impl std::fmt::Display for JFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path.display())
    }
}

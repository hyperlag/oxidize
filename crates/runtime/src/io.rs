#![allow(non_snake_case)]
//! Java I/O runtime types.
//!
//! Provides Rust equivalents for:
//! - `java.io.File` → [`JFile`]
//! - `java.io.BufferedReader` → [`JBufferedReader`]
//! - `java.io.BufferedWriter` → [`JBufferedWriter`]
//! - `java.io.PrintWriter` → [`JPrintWriter`]
//! - `java.io.FileReader` → used to construct [`JBufferedReader`]
//! - `java.io.FileWriter` → used to construct [`JPrintWriter`] / [`JBufferedWriter`]
//! - `java.io.FileInputStream` → [`JFileInputStream`]
//! - `java.io.FileOutputStream` → [`JFileOutputStream`]
//! - `java.io.InputStreamReader` → used to construct [`JBufferedReader`]
//! - `java.util.Scanner` → [`JScanner`]
//! - `java.nio.file.Files` → [`JFiles`] (static utility methods)
//! - `java.nio.file.Path` / `java.nio.file.Paths` → [`JPath`]

use crate::list::JList;
use crate::string::JString;
use std::io::{BufRead, Read, Write};
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
        let abs_path = if self.path.is_absolute() {
            self.path.clone()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(&self.path))
                .unwrap_or_else(|_| self.path.clone())
        };
        JString::from(abs_path.to_str().unwrap_or(""))
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

    /// Java `file.toPath()`.
    pub fn toPath(&self) -> JPath {
        JPath::of_pathbuf(self.path.clone())
    }

    /// Access the inner `PathBuf` (used by other I/O types).
    pub(crate) fn path_buf(&self) -> &PathBuf {
        &self.path
    }
}

impl std::fmt::Display for JFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path.display())
    }
}

// ─── JPath ────────────────────────────────────────────────────────────────────

/// Java `java.nio.file.Path` — an immutable path reference.
#[derive(Debug, Clone, Default)]
pub struct JPath {
    inner: PathBuf,
}

impl JPath {
    /// Create from a string (used by `Paths.get()`).
    pub fn get(path: JString) -> Self {
        Self {
            inner: PathBuf::from(path.as_str()),
        }
    }

    /// Create from a `PathBuf` (internal).
    pub(crate) fn of_pathbuf(p: PathBuf) -> Self {
        Self { inner: p }
    }

    /// Java `path.toString()`.
    pub fn toString(&self) -> JString {
        JString::from(self.inner.to_str().unwrap_or(""))
    }

    /// Java `path.toFile()`.
    pub fn toFile(&self) -> JFile {
        JFile::new(self.toString())
    }

    /// Java `path.getFileName()`.
    pub fn getFileName(&self) -> JPath {
        JPath::of_pathbuf(
            self.inner
                .file_name()
                .map(PathBuf::from)
                .unwrap_or_default(),
        )
    }

    /// Java `path.getParent()`.
    pub fn getParent(&self) -> JPath {
        JPath::of_pathbuf(self.inner.parent().map(PathBuf::from).unwrap_or_default())
    }

    /// Java `path.resolve(other)`.
    pub fn resolve(&self, other: JString) -> JPath {
        JPath::of_pathbuf(self.inner.join(other.as_str()))
    }

    /// Java `path.toAbsolutePath()`.
    pub fn toAbsolutePath(&self) -> JPath {
        if self.inner.is_absolute() {
            self.clone()
        } else {
            let abs = std::env::current_dir()
                .map(|cwd| cwd.join(&self.inner))
                .unwrap_or_else(|_| self.inner.clone());
            JPath::of_pathbuf(abs)
        }
    }

    /// Access the inner `PathBuf`.
    pub(crate) fn path_buf(&self) -> &PathBuf {
        &self.inner
    }
}

impl std::fmt::Display for JPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner.display())
    }
}

/// Java `java.nio.file.Paths` — static factory (delegates to `JPath::get`).
pub struct JPaths;

impl JPaths {
    /// Java `Paths.get(path)`.
    pub fn get(path: JString) -> JPath {
        JPath::get(path)
    }
}

// ─── JBufferedReader ──────────────────────────────────────────────────────────

/// Java `java.io.BufferedReader` — line-oriented character input.
///
/// Wraps a `BufReader` over either a file or stdin.
pub struct JBufferedReader {
    inner: Box<dyn BufRead>,
}

impl std::fmt::Debug for JBufferedReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JBufferedReader").finish()
    }
}

impl Default for JBufferedReader {
    fn default() -> Self {
        Self::new_stdin()
    }
}

impl JBufferedReader {
    /// Construct from a `JFileReader` (i.e. `new BufferedReader(new FileReader(...))`).
    pub fn from_reader(reader: JFileReader) -> Self {
        Self {
            inner: Box::new(std::io::BufReader::new(reader.into_file())),
        }
    }

    /// Construct from an `InputStreamReader(System.in)` — reads from stdin.
    pub fn new_stdin() -> Self {
        Self {
            inner: Box::new(std::io::BufReader::new(std::io::stdin())),
        }
    }

    /// Construct from any `BufRead` implementor (e.g. a wrapped process stdout
    /// pipe).  Used by [`JProcess::getInputStream`].
    pub fn from_bufreader<R: std::io::BufRead + 'static>(reader: R) -> Self {
        Self {
            inner: Box::new(reader),
        }
    }

    /// Construct from a raw `String` (e.g. captured process output).
    pub fn from_raw_string(s: String) -> Self {
        Self {
            inner: Box::new(std::io::Cursor::new(s)),
        }
    }

    /// Construct from a `JStringReader` (i.e. `new BufferedReader(new StringReader(...))`).
    pub fn from_string_reader(sr: JStringReader) -> Self {
        Self {
            inner: Box::new(std::io::BufReader::new(sr.cursor)),
        }
    }

    /// Java `br.readLine()` — returns an empty `JString` at EOF.
    pub fn readLine(&mut self) -> JString {
        let mut line = String::new();
        match self.inner.read_line(&mut line) {
            Ok(0) => JString::from("null"),
            Ok(_) => {
                // Strip trailing newline to match Java behaviour
                if line.ends_with('\n') {
                    line.pop();
                    if line.ends_with('\r') {
                        line.pop();
                    }
                }
                JString::from(line.as_str())
            }
            Err(_) => JString::from("null"),
        }
    }

    /// Java `br.read()` — reads a single character, returns -1 at EOF.
    pub fn read(&mut self) -> i32 {
        let mut buf = [0u8; 4];
        match self.inner.read(&mut buf[..1]) {
            Ok(0) | Err(_) => -1,
            Ok(_) => buf[0] as i32,
        }
    }

    /// Java `br.ready()` — returns true if the stream has data available.
    pub fn ready(&self) -> bool {
        // Conservative: always true for files (BufReader buffers ahead)
        true
    }

    /// Java `br.close()`.
    pub fn close(&mut self) {
        // Drop the inner reader (Rust handles cleanup automatically)
    }
}

// ─── JBufferedWriter ──────────────────────────────────────────────────────────

/// Java `java.io.BufferedWriter` — buffered character output.
pub struct JBufferedWriter {
    inner: Box<dyn Write>,
}

impl std::fmt::Debug for JBufferedWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JBufferedWriter").finish()
    }
}

impl Default for JBufferedWriter {
    fn default() -> Self {
        Self {
            inner: Box::new(std::io::BufWriter::new(std::io::stdout())),
        }
    }
}

impl JBufferedWriter {
    /// Construct from a `JFileWriter`.
    pub fn from_writer(writer: JFileWriter) -> Self {
        Self {
            inner: Box::new(std::io::BufWriter::new(writer.into_file())),
        }
    }

    /// Java `bw.write(str)`.
    pub fn write(&mut self, s: JString) {
        let _ = self.inner.write_all(s.as_str().as_bytes());
    }

    /// Java `bw.newLine()`.
    pub fn newLine(&mut self) {
        let _ = self.inner.write_all(b"\n");
    }

    /// Java `bw.flush()`.
    pub fn flush(&mut self) {
        let _ = self.inner.flush();
    }

    /// Java `bw.close()`.
    pub fn close(&mut self) {
        let _ = self.inner.flush();
    }
}

// ─── JPrintWriter ─────────────────────────────────────────────────────────────

/// Java `java.io.PrintWriter` — formatted text output.
pub struct JPrintWriter {
    inner: Box<dyn Write>,
}

impl std::fmt::Debug for JPrintWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JPrintWriter").finish()
    }
}

impl Default for JPrintWriter {
    fn default() -> Self {
        Self {
            inner: Box::new(std::io::stdout()),
        }
    }
}

impl JPrintWriter {
    /// Construct from a file path string: `new PrintWriter("output.txt")`.
    pub fn new_from_path(path: JString) -> Self {
        let file = std::fs::File::create(path.as_str())
            .unwrap_or_else(|e| panic!("JException:IOException:{}", e));
        Self {
            inner: Box::new(std::io::BufWriter::new(file)),
        }
    }

    /// Construct from a `JFileWriter`.
    pub fn from_writer(writer: JFileWriter) -> Self {
        Self {
            inner: Box::new(std::io::BufWriter::new(writer.into_file())),
        }
    }

    /// Construct from a `JFile`.
    pub fn from_file(file: &JFile) -> Self {
        let f = std::fs::File::create(file.path_buf())
            .unwrap_or_else(|e| panic!("JException:IOException:{}", e));
        Self {
            inner: Box::new(std::io::BufWriter::new(f)),
        }
    }

    /// Construct pointing at a `JStringWriter` (writes to in-memory buffer).
    ///
    /// The `JStringWriter` and this `JPrintWriter` share the same buffer via
    /// `Rc<RefCell<String>>`, so content written through `pw` is immediately
    /// visible in `sw.toString()`.
    pub fn from_string_writer(sw: &JStringWriter) -> Self {
        Self {
            inner: Box::new(SharedStringBuf(sw.shared_buf())),
        }
    }

    /// Java `pw.println(x)`.
    pub fn println(&mut self, s: JString) {
        let _ = writeln!(self.inner, "{}", s);
    }

    /// Java `pw.println()` (no args).
    pub fn println_empty(&mut self) {
        let _ = writeln!(self.inner);
    }

    /// Java `pw.print(x)`.
    pub fn print(&mut self, s: JString) {
        let _ = write!(self.inner, "{}", s);
    }

    /// Java `pw.printf(fmt, args)` — simplified, just writes the format string.
    pub fn printf(&mut self, s: JString) {
        let _ = write!(self.inner, "{}", s);
    }

    /// Java `pw.write(str)`.
    pub fn write(&mut self, s: JString) {
        let _ = self.inner.write_all(s.as_str().as_bytes());
    }

    /// Java `pw.flush()`.
    pub fn flush(&mut self) {
        let _ = self.inner.flush();
    }

    /// Java `pw.close()`.
    pub fn close(&mut self) {
        let _ = self.inner.flush();
    }
}

// ─── JFileReader ──────────────────────────────────────────────────────────────

/// Java `java.io.FileReader` — character input from a file.
///
/// Typically used as: `new BufferedReader(new FileReader("path"))`.
pub struct JFileReader {
    file: std::fs::File,
}

impl std::fmt::Debug for JFileReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JFileReader").finish()
    }
}

impl Default for JFileReader {
    fn default() -> Self {
        let null_path = if cfg!(windows) { "NUL" } else { "/dev/null" };
        Self {
            file: std::fs::File::open(null_path)
                .unwrap_or_else(|e| panic!("JException:IOException:{}", e)),
        }
    }
}

impl JFileReader {
    /// Java `new FileReader(path)`.
    pub fn new(path: JString) -> Self {
        let file = std::fs::File::open(path.as_str())
            .unwrap_or_else(|e| panic!("JException:FileNotFoundException:{}", e));
        Self { file }
    }

    /// Java `new FileReader(file)`.
    pub fn from_file(file: &JFile) -> Self {
        let f = std::fs::File::open(file.path_buf())
            .unwrap_or_else(|e| panic!("JException:FileNotFoundException:{}", e));
        Self { file: f }
    }

    /// Consume and return the inner `File` for wrapping in a `BufReader`.
    pub(crate) fn into_file(self) -> std::fs::File {
        self.file
    }

    /// Java `fr.close()`.
    pub fn close(self) {
        // File is closed when dropped
    }
}

// ─── JFileWriter ──────────────────────────────────────────────────────────────

/// Java `java.io.FileWriter` — character output to a file.
///
/// Typically used as: `new PrintWriter(new FileWriter("path"))`.
pub struct JFileWriter {
    file: std::fs::File,
}

impl std::fmt::Debug for JFileWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JFileWriter").finish()
    }
}

impl Default for JFileWriter {
    fn default() -> Self {
        let null_path = if cfg!(windows) { "NUL" } else { "/dev/null" };
        Self {
            file: std::fs::File::create(null_path)
                .unwrap_or_else(|e| panic!("JException:IOException:{}", e)),
        }
    }
}

impl JFileWriter {
    /// Java `new FileWriter(path)`.
    pub fn new(path: JString) -> Self {
        let file = std::fs::File::create(path.as_str())
            .unwrap_or_else(|e| panic!("JException:IOException:{}", e));
        Self { file }
    }

    /// Java `new FileWriter(path, append)`.
    pub fn new_append(path: JString, append: bool) -> Self {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .append(append)
            .truncate(!append)
            .open(path.as_str())
            .unwrap_or_else(|e| panic!("JException:IOException:{}", e));
        Self { file }
    }

    /// Consume and return the inner `File`.
    pub(crate) fn into_file(self) -> std::fs::File {
        self.file
    }

    /// Java `fw.write(str)`.
    pub fn write(&mut self, s: JString) {
        let _ = self.file.write_all(s.as_str().as_bytes());
    }

    /// Java `fw.close()`.
    pub fn close(&mut self) {
        let _ = self.file.flush();
    }
}

// ─── JFileInputStream ─────────────────────────────────────────────────────────

/// Java `java.io.FileInputStream` — byte-level file input.
pub struct JFileInputStream {
    file: std::fs::File,
}

impl std::fmt::Debug for JFileInputStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JFileInputStream").finish()
    }
}

impl Default for JFileInputStream {
    fn default() -> Self {
        let null_path = if cfg!(windows) { "NUL" } else { "/dev/null" };
        Self {
            file: std::fs::File::open(null_path)
                .unwrap_or_else(|e| panic!("JException:IOException:{}", e)),
        }
    }
}

impl JFileInputStream {
    /// Java `new FileInputStream(path)`.
    pub fn new(path: JString) -> Self {
        let file = std::fs::File::open(path.as_str())
            .unwrap_or_else(|e| panic!("JException:FileNotFoundException:{}", e));
        Self { file }
    }

    /// Java `new FileInputStream(file)`.
    pub fn from_file(file: &JFile) -> Self {
        let f = std::fs::File::open(file.path_buf())
            .unwrap_or_else(|e| panic!("JException:FileNotFoundException:{}", e));
        Self { file: f }
    }

    /// Java `fis.read()` — reads a single byte, returns -1 at EOF.
    pub fn read(&mut self) -> i32 {
        let mut buf = [0u8; 1];
        match self.file.read(&mut buf) {
            Ok(0) | Err(_) => -1,
            Ok(_) => buf[0] as i32,
        }
    }

    /// Java `fis.read(byte[])` — reads into a byte array, returns bytes read or -1.
    pub fn read_into(&mut self, buf: &mut [u8]) -> i32 {
        match self.file.read(buf) {
            Ok(0) | Err(_) => -1,
            Ok(n) => n as i32,
        }
    }

    /// Java `fis.available()`.
    pub fn available(&self) -> i32 {
        // Conservative estimate
        0
    }

    /// Java `fis.close()`.
    pub fn close(self) {
        // File is closed when dropped
    }
}

// ─── JFileOutputStream ────────────────────────────────────────────────────────

/// Java `java.io.FileOutputStream` — byte-level file output.
pub struct JFileOutputStream {
    file: std::fs::File,
}

impl std::fmt::Debug for JFileOutputStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JFileOutputStream").finish()
    }
}

impl Default for JFileOutputStream {
    fn default() -> Self {
        let null_path = if cfg!(windows) { "NUL" } else { "/dev/null" };
        Self {
            file: std::fs::File::create(null_path)
                .unwrap_or_else(|e| panic!("JException:IOException:{}", e)),
        }
    }
}

impl JFileOutputStream {
    /// Java `new FileOutputStream(path)`.
    pub fn new(path: JString) -> Self {
        let file = std::fs::File::create(path.as_str())
            .unwrap_or_else(|e| panic!("JException:IOException:{}", e));
        Self { file }
    }

    /// Java `new FileOutputStream(path, append)`.
    pub fn new_append(path: JString, append: bool) -> Self {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .append(append)
            .truncate(!append)
            .open(path.as_str())
            .unwrap_or_else(|e| panic!("JException:IOException:{}", e));
        Self { file }
    }

    /// Java `fos.write(int b)` — writes a single byte.
    pub fn write_byte(&mut self, b: i32) {
        let _ = self.file.write_all(&[b as u8]);
    }

    /// Java `fos.write(byte[])`.
    pub fn write_bytes(&mut self, buf: &[u8]) {
        let _ = self.file.write_all(buf);
    }

    /// Java `fos.flush()`.
    pub fn flush(&mut self) {
        let _ = self.file.flush();
    }

    /// Java `fos.close()`.
    pub fn close(&mut self) {
        let _ = self.file.flush();
    }
}

// ─── JScanner ─────────────────────────────────────────────────────────────────

/// Java `java.util.Scanner` — simple text parsing.
#[derive(Default)]
pub struct JScanner {
    lines: Vec<String>,
    pos: usize,
}

impl std::fmt::Debug for JScanner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JScanner").field("pos", &self.pos).finish()
    }
}

impl JScanner {
    /// Java `new Scanner(System.in)` — reads all of stdin eagerly.
    pub fn new_stdin() -> Self {
        let mut content = String::new();
        let _ = std::io::stdin().read_to_string(&mut content);
        Self::from_string_content(content)
    }

    /// Java `new Scanner(file)` — reads from a file.
    pub fn from_file(file: &JFile) -> Self {
        let content = std::fs::read_to_string(file.path_buf())
            .unwrap_or_else(|e| panic!("JException:FileNotFoundException:{}", e));
        Self::from_string_content(content)
    }

    /// Java `new Scanner(string)` — reads from a string.
    pub fn from_string(s: JString) -> Self {
        Self::from_string_content(s.as_str().to_string())
    }

    fn from_string_content(content: String) -> Self {
        // Split into tokens (whitespace-delimited), but keep line structure
        // for nextLine(). Store raw lines.
        let lines: Vec<String> = content.lines().map(String::from).collect();
        Self { lines, pos: 0 }
    }

    /// Java `scanner.hasNextLine()`.
    pub fn hasNextLine(&self) -> bool {
        self.pos < self.lines.len()
    }

    /// Java `scanner.nextLine()`.
    pub fn nextLine(&mut self) -> JString {
        if self.pos < self.lines.len() {
            let line = &self.lines[self.pos];
            self.pos += 1;
            JString::from(line.as_str())
        } else {
            panic!("JException:NoSuchElementException:No line found");
        }
    }

    /// Java `scanner.hasNext()`.
    pub fn hasNext(&self) -> bool {
        self.current_tokens().is_some()
    }

    /// Java `scanner.next()`.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> JString {
        if let Some(token) = self.next_token() {
            JString::from(token.as_str())
        } else {
            panic!("JException:NoSuchElementException:No token found");
        }
    }

    /// Java `scanner.hasNextInt()`.
    pub fn hasNextInt(&self) -> bool {
        self.current_tokens()
            .map(|t| t.parse::<i32>().is_ok())
            .unwrap_or(false)
    }

    /// Java `scanner.nextInt()`.
    pub fn nextInt(&mut self) -> i32 {
        let token = self
            .next_token()
            .unwrap_or_else(|| panic!("JException:NoSuchElementException:No int found"));
        token
            .parse()
            .unwrap_or_else(|_| panic!("JException:InputMismatchException:{}", token))
    }

    /// Java `scanner.nextDouble()`.
    pub fn nextDouble(&mut self) -> f64 {
        let token = self
            .next_token()
            .unwrap_or_else(|| panic!("JException:NoSuchElementException:No double found"));
        token
            .parse()
            .unwrap_or_else(|_| panic!("JException:InputMismatchException:{}", token))
    }

    /// Java `scanner.nextLong()`.
    pub fn nextLong(&mut self) -> i64 {
        let token = self
            .next_token()
            .unwrap_or_else(|| panic!("JException:NoSuchElementException:No long found"));
        token
            .parse()
            .unwrap_or_else(|_| panic!("JException:InputMismatchException:{}", token))
    }

    /// Java `scanner.close()`.
    pub fn close(&mut self) {
        // No-op: all content already read
    }

    /// Get the next whitespace-delimited token from the current line position.
    fn next_token(&mut self) -> Option<String> {
        while self.pos < self.lines.len() {
            let line = self.lines[self.pos].clone();
            let mut iter = line.split_whitespace();
            if let Some(first) = iter.next() {
                let token = first.to_string();
                let remaining_tokens: Vec<&str> = iter.collect();
                if remaining_tokens.is_empty() {
                    // No more tokens on this line; advance to the next line.
                    self.pos += 1;
                } else {
                    // Preserve remaining tokens for subsequent calls.
                    self.lines[self.pos] = remaining_tokens.join(" ");
                }
                return Some(token);
            }
            // Line contained no tokens; move to the next line.
            self.pos += 1;
        }
        None
    }

    /// Peek at the next token without consuming.
    fn current_tokens(&self) -> Option<String> {
        let mut pos = self.pos;
        while pos < self.lines.len() {
            let tokens: Vec<&str> = self.lines[pos].split_whitespace().collect();
            if !tokens.is_empty() {
                return Some(tokens[0].to_string());
            }
            pos += 1;
        }
        None
    }
}

// ─── JFiles ───────────────────────────────────────────────────────────────────

/// Java `java.nio.file.Files` — static utility methods.
pub struct JFiles;

impl JFiles {
    /// Java `Files.readString(path)`.
    pub fn readString(path: &JPath) -> JString {
        let content = std::fs::read_to_string(path.path_buf())
            .unwrap_or_else(|e| panic!("JException:IOException:{}", e));
        JString::from(content.as_str())
    }

    /// Java `Files.writeString(path, content)`.
    pub fn writeString(path: &JPath, content: JString) -> JPath {
        std::fs::write(path.path_buf(), content.as_str())
            .unwrap_or_else(|e| panic!("JException:IOException:{}", e));
        path.clone()
    }

    /// Java `Files.readAllLines(path)`.
    pub fn readAllLines(path: &JPath) -> JList<JString> {
        let content = std::fs::read_to_string(path.path_buf())
            .unwrap_or_else(|e| panic!("JException:IOException:{}", e));
        let mut list = JList::new();
        for line in content.lines() {
            list.add(JString::from(line));
        }
        list
    }

    /// Java `Files.write(path, lines)`.
    pub fn write_lines(path: &JPath, lines: &JList<JString>) -> JPath {
        let mut content = String::new();
        for i in 0..lines.size() {
            if i > 0 {
                content.push('\n');
            }
            content.push_str(lines.get(i).as_str());
        }
        content.push('\n');
        std::fs::write(path.path_buf(), content)
            .unwrap_or_else(|e| panic!("JException:IOException:{}", e));
        path.clone()
    }

    /// Java `Files.exists(path)`.
    pub fn exists(path: &JPath) -> bool {
        path.path_buf().exists()
    }

    /// Java `Files.isDirectory(path)`.
    pub fn isDirectory(path: &JPath) -> bool {
        path.path_buf().is_dir()
    }

    /// Java `Files.isRegularFile(path)`.
    pub fn isRegularFile(path: &JPath) -> bool {
        path.path_buf().is_file()
    }

    /// Java `Files.size(path)`.
    pub fn size(path: &JPath) -> i64 {
        std::fs::metadata(path.path_buf())
            .map(|m| m.len() as i64)
            .unwrap_or_else(|e| panic!("JException:IOException:{}", e))
    }

    /// Java `Files.delete(path)`.
    pub fn delete(path: &JPath) {
        let p = path.path_buf();
        if p.is_dir() {
            std::fs::remove_dir(p).unwrap_or_else(|e| panic!("JException:IOException:{}", e));
        } else {
            std::fs::remove_file(p).unwrap_or_else(|e| panic!("JException:IOException:{}", e));
        }
    }

    /// Java `Files.deleteIfExists(path)`.
    pub fn deleteIfExists(path: &JPath) -> bool {
        let p = path.path_buf();
        if p.exists() {
            if p.is_dir() {
                std::fs::remove_dir(p).is_ok()
            } else {
                std::fs::remove_file(p).is_ok()
            }
        } else {
            false
        }
    }

    /// Java `Files.createDirectory(path)`.
    pub fn createDirectory(path: &JPath) -> JPath {
        std::fs::create_dir(path.path_buf())
            .unwrap_or_else(|e| panic!("JException:IOException:{}", e));
        path.clone()
    }

    /// Java `Files.createDirectories(path)`.
    pub fn createDirectories(path: &JPath) -> JPath {
        std::fs::create_dir_all(path.path_buf())
            .unwrap_or_else(|e| panic!("JException:IOException:{}", e));
        path.clone()
    }

    /// Java `Files.copy(source, target)`.
    pub fn copy(source: &JPath, target: &JPath) -> JPath {
        std::fs::copy(source.path_buf(), target.path_buf())
            .unwrap_or_else(|e| panic!("JException:IOException:{}", e));
        target.clone()
    }

    /// Java `Files.move(source, target)`.
    pub fn move_path(source: &JPath, target: &JPath) -> JPath {
        std::fs::rename(source.path_buf(), target.path_buf())
            .unwrap_or_else(|e| panic!("JException:IOException:{}", e));
        target.clone()
    }
}

// ─── JStringWriter ────────────────────────────────────────────────────────────

/// Java `java.io.StringWriter` — in-memory character writer.
///
/// Writes accumulate in a shared `Rc<RefCell<String>>` buffer so that a
/// `JPrintWriter` constructed from this `JStringWriter` writes into the same
/// buffer and the content is visible via `sw.toString()` afterwards.
#[derive(Debug, Clone, Default)]
pub struct JStringWriter {
    buf: std::rc::Rc<std::cell::RefCell<String>>,
}

/// Private adapter that implements `Write` by forwarding into the shared buffer.
struct SharedStringBuf(std::rc::Rc<std::cell::RefCell<String>>);

impl Write for SharedStringBuf {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let s = String::from_utf8_lossy(buf);
        self.0.borrow_mut().push_str(&s);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl JStringWriter {
    /// Java `new StringWriter()`.
    pub fn new() -> Self {
        Self {
            buf: std::rc::Rc::new(std::cell::RefCell::new(String::new())),
        }
    }

    /// Java `sw.write(str)`.
    pub fn write(&mut self, s: JString) {
        self.buf.borrow_mut().push_str(s.as_str());
    }

    /// Java `sw.toString()`.
    pub fn toString(&self) -> JString {
        JString::from(self.buf.borrow().as_str())
    }

    /// Java `sw.getBuffer()` — returns current content as a `JString`.
    pub fn getBuffer(&self) -> JString {
        self.toString()
    }

    /// Java `sw.flush()` — no-op for in-memory.
    pub fn flush(&mut self) {}

    /// Java `sw.close()` — no-op for in-memory.
    pub fn close(&mut self) {}

    /// Shared buffer reference for use by `JPrintWriter::from_string_writer`.
    pub(crate) fn shared_buf(&self) -> std::rc::Rc<std::cell::RefCell<String>> {
        self.buf.clone()
    }
}

impl Write for JStringWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let s = String::from_utf8_lossy(buf);
        self.buf.borrow_mut().push_str(&s);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl std::fmt::Display for JStringWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.buf.borrow())
    }
}

// ─── JStringReader ────────────────────────────────────────────────────────────

/// Java `java.io.StringReader` — in-memory character reader.
#[derive(Debug, Clone)]
pub struct JStringReader {
    cursor: std::io::Cursor<Vec<u8>>,
}

impl Default for JStringReader {
    fn default() -> Self {
        Self::new(JString::from(""))
    }
}

impl JStringReader {
    /// Java `new StringReader(str)`.
    pub fn new(s: JString) -> Self {
        Self {
            cursor: std::io::Cursor::new(s.as_str().as_bytes().to_vec()),
        }
    }

    /// Java `sr.read()` — reads a single character, or -1 at EOF.
    pub fn read(&mut self) -> i32 {
        let mut buf = [0u8; 1];
        match self.cursor.read(&mut buf) {
            Ok(0) | Err(_) => -1,
            Ok(_) => buf[0] as i32,
        }
    }

    /// Java `sr.close()` — no-op.
    pub fn close(&mut self) {}
}

// ─── JByteArrayOutputStream ───────────────────────────────────────────────────

/// Java `java.io.ByteArrayOutputStream` — in-memory byte writer.
#[derive(Debug, Clone, Default)]
pub struct JByteArrayOutputStream {
    buf: Vec<u8>,
}

impl JByteArrayOutputStream {
    /// Java `new ByteArrayOutputStream()`.
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// Java `baos.write(int b)` — appends a single byte.
    pub fn write(&mut self, b: i32) {
        self.buf.push(b as u8);
    }

    /// Java `baos.size()` — number of bytes written.
    pub fn size(&self) -> i32 {
        self.buf.len() as i32
    }

    /// Java `baos.toString()` — interprets bytes as UTF-8 (lossy).
    pub fn toString(&self) -> JString {
        JString::from(String::from_utf8_lossy(&self.buf).as_ref())
    }

    /// Java `baos.toByteArray()` — returns a copy as a `JArray<i32>`.
    pub fn toByteArray(&self) -> crate::array::JArray<i32> {
        let bytes: Vec<i32> = self.buf.iter().map(|&b| b as i32).collect();
        crate::array::JArray::from_vec(bytes)
    }

    /// Java `baos.reset()` — clears the buffer.
    pub fn reset(&mut self) {
        self.buf.clear();
    }

    /// Java `baos.flush()` — no-op.
    pub fn flush(&mut self) {}

    /// Java `baos.close()` — no-op.
    pub fn close(&mut self) {}
}

impl Write for JByteArrayOutputStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buf.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl std::fmt::Display for JByteArrayOutputStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", String::from_utf8_lossy(&self.buf))
    }
}

// ─── JByteArrayInputStream ────────────────────────────────────────────────────

/// Java `java.io.ByteArrayInputStream` — in-memory byte reader.
#[derive(Debug, Clone)]
pub struct JByteArrayInputStream {
    cursor: std::io::Cursor<Vec<u8>>,
}

impl Default for JByteArrayInputStream {
    fn default() -> Self {
        Self::new(crate::array::JArray::from_vec(vec![]))
    }
}

impl JByteArrayInputStream {
    /// Java `new ByteArrayInputStream(byte[])` — the `JArray<i8>` holds the bytes.
    pub fn new(data: crate::array::JArray<i8>) -> Self {
        let bytes: Vec<u8> = (0..data.length()).map(|i| data.get(i) as u8).collect();
        Self {
            cursor: std::io::Cursor::new(bytes),
        }
    }

    /// Java `bais.read()` — reads a single byte, returns -1 at EOF.
    pub fn read(&mut self) -> i32 {
        let mut buf = [0u8; 1];
        match self.cursor.read(&mut buf) {
            Ok(0) | Err(_) => -1,
            Ok(_) => buf[0] as i32,
        }
    }

    /// Java `bais.available()`.
    pub fn available(&self) -> i32 {
        let pos = self.cursor.position() as usize;
        let len = self.cursor.get_ref().len();
        (len.saturating_sub(pos)) as i32
    }

    /// Java `bais.close()` — no-op.
    pub fn close(&mut self) {}

    /// Internal: read all remaining bytes (used by `JResourceBundle`).
    pub(crate) fn read_all_bytes(&mut self) -> Vec<u8> {
        let pos = self.cursor.position() as usize;
        self.cursor.get_ref()[pos..].to_vec()
    }
}

impl Read for JByteArrayInputStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.cursor.read(buf)
    }
}

// ─── Polymorphic I/O base types (Java abstract class → Rust enum) ─────────────

/// Java `java.io.InputStream` — polymorphic byte input.
///
/// Wraps concrete `FileInputStream` and `ByteArrayInputStream` so that methods
/// accepting `InputStream` work with any implementation.
#[derive(Debug)]
pub enum JInputStream {
    File(JFileInputStream),
    ByteArray(JByteArrayInputStream),
}

impl Clone for JInputStream {
    fn clone(&self) -> Self {
        match self {
            Self::ByteArray(b) => Self::ByteArray(b.clone()),
            _ => Self::default(),
        }
    }
}

impl Default for JInputStream {
    fn default() -> Self {
        Self::File(JFileInputStream::default())
    }
}

impl JInputStream {
    /// Java `is.read()` — reads a single byte, returns -1 at EOF.
    pub fn read(&mut self) -> i32 {
        match self {
            Self::File(f) => f.read(),
            Self::ByteArray(b) => b.read(),
        }
    }

    /// Java `is.available()`.
    pub fn available(&self) -> i32 {
        match self {
            Self::File(f) => f.available(),
            Self::ByteArray(b) => b.available(),
        }
    }

    /// Java `is.close()`.
    pub fn close(&mut self) {}
}

impl std::fmt::Display for JInputStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "InputStream")
    }
}

impl From<JFileInputStream> for JInputStream {
    fn from(f: JFileInputStream) -> Self {
        Self::File(f)
    }
}

impl From<JByteArrayInputStream> for JInputStream {
    fn from(b: JByteArrayInputStream) -> Self {
        Self::ByteArray(b)
    }
}

/// Java `java.io.OutputStream` — polymorphic byte output.
#[derive(Debug)]
pub enum JOutputStream {
    File(JFileOutputStream),
    ByteArray(JByteArrayOutputStream),
}

impl Clone for JOutputStream {
    fn clone(&self) -> Self {
        match self {
            Self::ByteArray(b) => Self::ByteArray(b.clone()),
            _ => Self::default(),
        }
    }
}

impl Default for JOutputStream {
    fn default() -> Self {
        Self::File(JFileOutputStream::default())
    }
}

impl JOutputStream {
    /// Java `os.write(int b)` — writes a single byte.
    pub fn write(&mut self, b: i32) {
        match self {
            Self::File(f) => {
                let _ = f.file.write_all(&[b as u8]);
            }
            Self::ByteArray(ba) => ba.write(b),
        }
    }

    /// Java `os.flush()`.
    pub fn flush(&mut self) {
        match self {
            Self::File(f) => {
                let _ = f.file.flush();
            }
            Self::ByteArray(ba) => ba.flush(),
        }
    }

    /// Java `os.close()`.
    pub fn close(&mut self) {
        self.flush();
    }
}

impl std::fmt::Display for JOutputStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OutputStream")
    }
}

impl From<JFileOutputStream> for JOutputStream {
    fn from(f: JFileOutputStream) -> Self {
        Self::File(f)
    }
}

impl From<JByteArrayOutputStream> for JOutputStream {
    fn from(b: JByteArrayOutputStream) -> Self {
        Self::ByteArray(b)
    }
}

/// Java `java.io.Reader` — polymorphic character input.
///
/// Wraps concrete reader types so that methods accepting `Reader` work with
/// `FileReader`, `StringReader`, and `BufferedReader`.
#[derive(Debug)]
pub enum JReader {
    File(JFileReader),
    String(JStringReader),
    Buffered(JBufferedReader),
}

impl Default for JReader {
    fn default() -> Self {
        Self::File(JFileReader::default())
    }
}

impl Clone for JReader {
    fn clone(&self) -> Self {
        // Readers are not truly clonable; provide a default fallback.
        Self::default()
    }
}

impl JReader {
    /// Java `r.read()` — reads a single character, returns -1 at EOF.
    pub fn read(&mut self) -> i32 {
        match self {
            Self::File(_) => -1, // FileReader doesn't expose read directly
            Self::String(s) => s.read(),
            Self::Buffered(b) => b.read(),
        }
    }

    /// Java `r.close()`.
    pub fn close(&mut self) {}

    /// Convert into a `JBufferedReader` for wrapping.
    pub fn into_buffered_reader(self) -> JBufferedReader {
        match self {
            Self::File(fr) => JBufferedReader::from_reader(fr),
            Self::String(sr) => JBufferedReader::from_bufreader(std::io::BufReader::new(sr.cursor)),
            Self::Buffered(br) => br,
        }
    }
}

impl std::fmt::Display for JReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Reader")
    }
}

impl From<JFileReader> for JReader {
    fn from(f: JFileReader) -> Self {
        Self::File(f)
    }
}

impl From<JStringReader> for JReader {
    fn from(s: JStringReader) -> Self {
        Self::String(s)
    }
}

impl From<JBufferedReader> for JReader {
    fn from(b: JBufferedReader) -> Self {
        Self::Buffered(b)
    }
}

/// Java `java.io.Writer` — polymorphic character output.
///
/// Wraps concrete writer types so that methods accepting `Writer` work with
/// `FileWriter`, `StringWriter`, `BufferedWriter`, and `PrintWriter`.
#[derive(Debug)]
pub enum JWriter {
    File(JFileWriter),
    String(JStringWriter),
    Buffered(JBufferedWriter),
    Print(JPrintWriter),
}

impl Default for JWriter {
    fn default() -> Self {
        Self::File(JFileWriter::default())
    }
}

impl Clone for JWriter {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl JWriter {
    /// Java `w.write(str)`.
    pub fn write(&mut self, s: JString) {
        match self {
            Self::File(fw) => fw.write(s),
            Self::String(sw) => sw.write(s),
            Self::Buffered(bw) => bw.write(s),
            Self::Print(pw) => pw.write(s),
        }
    }

    /// Java `w.flush()`.
    pub fn flush(&mut self) {
        match self {
            Self::File(fw) => fw.close(),
            Self::String(sw) => sw.flush(),
            Self::Buffered(bw) => bw.flush(),
            Self::Print(pw) => pw.flush(),
        }
    }

    /// Java `w.close()`.
    pub fn close(&mut self) {
        self.flush();
    }

    /// Convert into a `JBufferedWriter`.
    pub fn into_buffered_writer(self) -> JBufferedWriter {
        match self {
            Self::File(fw) => JBufferedWriter::from_writer(fw),
            Self::Buffered(bw) => bw,
            _ => JBufferedWriter::default(),
        }
    }
}

impl std::fmt::Display for JWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Writer")
    }
}

impl From<JFileWriter> for JWriter {
    fn from(f: JFileWriter) -> Self {
        Self::File(f)
    }
}

impl From<JStringWriter> for JWriter {
    fn from(s: JStringWriter) -> Self {
        Self::String(s)
    }
}

impl From<JBufferedWriter> for JWriter {
    fn from(b: JBufferedWriter) -> Self {
        Self::Buffered(b)
    }
}

impl From<JPrintWriter> for JWriter {
    fn from(p: JPrintWriter) -> Self {
        Self::Print(p)
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jpath_basic() {
        let p = JPath::get(JString::from("foo/bar.txt"));
        assert_eq!(p.toString().as_str(), "foo/bar.txt");
        assert_eq!(p.getFileName().toString().as_str(), "bar.txt");
    }

    #[test]
    fn jpath_resolve() {
        let base = JPath::get(JString::from("/tmp"));
        let resolved = base.resolve(JString::from("test.txt"));
        assert_eq!(resolved.toString().as_str(), "/tmp/test.txt");
    }

    #[test]
    fn jscanner_from_string() {
        let mut sc = JScanner::from_string(JString::from("hello\nworld\n42"));
        assert!(sc.hasNextLine());
        assert_eq!(sc.nextLine().as_str(), "hello");
        assert_eq!(sc.nextLine().as_str(), "world");
        assert_eq!(sc.nextLine().as_str(), "42");
        assert!(!sc.hasNextLine());
    }

    #[test]
    fn jfiles_write_read_roundtrip() {
        let dir = std::env::temp_dir().join("oxidize_test_jfiles");
        let _ = std::fs::create_dir_all(&dir);
        let p = JPath::of_pathbuf(dir.join("roundtrip.txt"));
        JFiles::writeString(&p, JString::from("hello world"));
        let content = JFiles::readString(&p);
        assert_eq!(content.as_str(), "hello world");
        assert!(JFiles::exists(&p));
        JFiles::delete(&p);
        assert!(!JFiles::exists(&p));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn jbuffered_reader_writer_roundtrip() {
        let dir = std::env::temp_dir().join("oxidize_test_bw");
        let _ = std::fs::create_dir_all(&dir);
        let path_str = dir.join("bw_test.txt");
        let ps = JString::from(path_str.to_str().unwrap());

        // Write with BufferedWriter
        let fw = JFileWriter::new(ps.clone());
        let mut bw = JBufferedWriter::from_writer(fw);
        bw.write(JString::from("line1"));
        bw.newLine();
        bw.write(JString::from("line2"));
        bw.newLine();
        bw.close();

        // Read with BufferedReader
        let fr = JFileReader::new(ps.clone());
        let mut br = JBufferedReader::from_reader(fr);
        assert_eq!(br.readLine().as_str(), "line1");
        assert_eq!(br.readLine().as_str(), "line2");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn jprintwriter_basic() {
        let dir = std::env::temp_dir().join("oxidize_test_pw");
        let _ = std::fs::create_dir_all(&dir);
        let path_str = dir.join("pw_test.txt");
        let ps = JString::from(path_str.to_str().unwrap());

        let mut pw = JPrintWriter::new_from_path(ps.clone());
        pw.println(JString::from("Hello"));
        pw.println(JString::from("World"));
        pw.close();

        let content = std::fs::read_to_string(&path_str).unwrap();
        assert_eq!(content, "Hello\nWorld\n");

        let _ = std::fs::remove_dir_all(&dir);
    }
}

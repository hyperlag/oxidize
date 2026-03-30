#![allow(non_snake_case)]
//! [`JURL`], [`JSocket`], [`JServerSocket`], [`JHttpURLConnection`] —
//! Rust runtime types for `java.net`.
//!
//! These wrap the Rust standard library networking primitives
//! (`std::net::TcpStream`, `std::net::TcpListener`) and provide
//! Java-compatible method signatures for transpiled code.

use crate::JString;
use std::fmt;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};

// ─── URL ─────────────────────────────────────────────────────────────────────

/// Rust equivalent of `java.net.URL`.
///
/// Stores the raw URL string and lazily parses components.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct JURL {
    raw: String,
}

impl JURL {
    /// `new URL(String)`
    pub fn new(spec: JString) -> Self {
        JURL {
            raw: spec.as_str().to_string(),
        }
    }

    /// `URL.getProtocol()`
    #[allow(non_snake_case)]
    pub fn getProtocol(&self) -> JString {
        if let Some(idx) = self.raw.find("://") {
            JString::from(&self.raw[..idx])
        } else {
            JString::from("")
        }
    }

    /// `URL.getHost()`
    #[allow(non_snake_case)]
    pub fn getHost(&self) -> JString {
        let after_scheme = self
            .raw
            .find("://")
            .map(|i| &self.raw[i + 3..])
            .unwrap_or(&self.raw);
        let host_port = after_scheme.split('/').next().unwrap_or("");
        // Strip userinfo if present
        let host_port = host_port
            .rfind('@')
            .map(|i| &host_port[i + 1..])
            .unwrap_or(host_port);
        // Strip port
        let host = if let Some(idx) = host_port.rfind(':') {
            &host_port[..idx]
        } else {
            host_port
        };
        JString::from(host)
    }

    /// `URL.getPort()` — returns -1 if no explicit port.
    #[allow(non_snake_case)]
    pub fn getPort(&self) -> i32 {
        let after_scheme = self
            .raw
            .find("://")
            .map(|i| &self.raw[i + 3..])
            .unwrap_or(&self.raw);
        let host_port = after_scheme.split('/').next().unwrap_or("");
        let host_port = host_port
            .rfind('@')
            .map(|i| &host_port[i + 1..])
            .unwrap_or(host_port);
        if let Some(idx) = host_port.rfind(':') {
            host_port[idx + 1..].parse::<i32>().unwrap_or(-1)
        } else {
            -1
        }
    }

    /// `URL.getDefaultPort()`
    #[allow(non_snake_case)]
    pub fn getDefaultPort(&self) -> i32 {
        match self.getProtocol().as_str() {
            "http" => 80,
            "https" => 443,
            "ftp" => 21,
            _ => -1,
        }
    }

    /// `URL.getPath()`
    #[allow(non_snake_case)]
    pub fn getPath(&self) -> JString {
        let after_scheme = self
            .raw
            .find("://")
            .map(|i| &self.raw[i + 3..])
            .unwrap_or(&self.raw);
        if let Some(slash_pos) = after_scheme.find('/') {
            let path_and_rest = &after_scheme[slash_pos..];
            let path = path_and_rest.split('?').next().unwrap_or(path_and_rest);
            let path = path.split('#').next().unwrap_or(path);
            JString::from(path)
        } else {
            JString::from("")
        }
    }

    /// `URL.getQuery()`
    #[allow(non_snake_case)]
    pub fn getQuery(&self) -> JString {
        if let Some(q) = self.raw.find('?') {
            let after_q = &self.raw[q + 1..];
            let query = after_q.split('#').next().unwrap_or(after_q);
            JString::from(query)
        } else {
            JString::from("")
        }
    }

    /// `URL.getFile()` — path + query string
    #[allow(non_snake_case)]
    pub fn getFile(&self) -> JString {
        let after_scheme = self
            .raw
            .find("://")
            .map(|i| &self.raw[i + 3..])
            .unwrap_or(&self.raw);
        if let Some(slash_pos) = after_scheme.find('/') {
            let file = &after_scheme[slash_pos..];
            let file = file.split('#').next().unwrap_or(file);
            JString::from(file)
        } else {
            JString::from("")
        }
    }

    /// `URL.getRef()` — fragment
    #[allow(non_snake_case)]
    pub fn getRef(&self) -> JString {
        if let Some(h) = self.raw.find('#') {
            JString::from(&self.raw[h + 1..])
        } else {
            JString::from("")
        }
    }

    /// `URL.toString()`
    #[allow(clippy::inherent_to_string)]
    pub fn toString(&self) -> JString {
        JString::from(self.raw.as_str())
    }

    /// `URL.toExternalForm()`
    #[allow(non_snake_case)]
    pub fn toExternalForm(&self) -> JString {
        self.toString()
    }
}

impl fmt::Display for JURL {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

// ─── Socket ──────────────────────────────────────────────────────────────────

/// Rust equivalent of `java.net.Socket`.
///
/// Wraps a `TcpStream`.
#[derive(Debug, Default)]
pub struct JSocket {
    stream: Option<TcpStream>,
    host: String,
    port: i32,
}

impl Clone for JSocket {
    fn clone(&self) -> Self {
        JSocket {
            stream: self.stream.as_ref().and_then(|s| s.try_clone().ok()),
            host: self.host.clone(),
            port: self.port,
        }
    }
}

impl JSocket {
    /// `new Socket(String host, int port)`
    pub fn new(host: JString, port: i32) -> Self {
        let addr = format!("{}:{}", host.as_str(), port);
        let stream = TcpStream::connect(&addr).ok();
        JSocket {
            stream,
            host: host.as_str().to_string(),
            port,
        }
    }

    /// `Socket.getPort()`
    #[allow(non_snake_case)]
    pub fn getPort(&self) -> i32 {
        self.port
    }

    /// `Socket.getLocalPort()`
    #[allow(non_snake_case)]
    pub fn getLocalPort(&self) -> i32 {
        self.stream
            .as_ref()
            .and_then(|s| s.local_addr().ok())
            .map(|a| a.port() as i32)
            .unwrap_or(-1)
    }

    /// `Socket.isClosed()`
    #[allow(non_snake_case)]
    pub fn isClosed(&self) -> bool {
        self.stream.is_none()
    }

    /// `Socket.isConnected()`
    #[allow(non_snake_case)]
    pub fn isConnected(&self) -> bool {
        self.stream.is_some()
    }

    /// `Socket.close()`
    pub fn close(&mut self) {
        self.stream = None;
    }

    /// Write bytes to the socket (used for output stream).
    pub fn write_bytes(&mut self, data: &[u8]) {
        if let Some(ref mut s) = self.stream {
            let _ = s.write_all(data);
            let _ = s.flush();
        }
    }

    /// Read all available bytes as a string (used for input stream).
    pub fn read_string(&mut self) -> JString {
        if let Some(ref mut s) = self.stream {
            let mut buf = String::new();
            let mut reader = BufReader::new(s);
            let _ = reader.read_line(&mut buf);
            JString::from(buf.trim_end())
        } else {
            JString::from("")
        }
    }

    /// Read all bytes until EOF.
    pub fn read_all(&mut self) -> JString {
        if let Some(ref mut s) = self.stream {
            let mut buf = String::new();
            let _ = s.read_to_string(&mut buf);
            JString::from(buf.as_str())
        } else {
            JString::from("")
        }
    }

    /// `Socket.getInputStream()` — returns self (for chaining in transpiled code).
    #[allow(non_snake_case)]
    pub fn getInputStream(&mut self) -> &mut Self {
        self
    }

    /// `Socket.getOutputStream()` — returns self (for chaining in transpiled code).
    #[allow(non_snake_case)]
    pub fn getOutputStream(&mut self) -> &mut Self {
        self
    }
}

// ─── ServerSocket ────────────────────────────────────────────────────────────

/// Rust equivalent of `java.net.ServerSocket`.
///
/// Wraps a `TcpListener`.
#[derive(Debug, Default)]
pub struct JServerSocket {
    listener: Option<TcpListener>,
    port: i32,
}

impl Clone for JServerSocket {
    fn clone(&self) -> Self {
        JServerSocket {
            listener: self.listener.as_ref().and_then(|l| l.try_clone().ok()),
            port: self.port,
        }
    }
}

impl JServerSocket {
    /// `new ServerSocket(int port)`
    pub fn new(port: i32) -> Self {
        let addr = format!("0.0.0.0:{port}");
        let listener = TcpListener::bind(&addr).ok();
        JServerSocket { listener, port }
    }

    /// `ServerSocket.accept()` — blocks until a client connects.
    pub fn accept(&self) -> JSocket {
        if let Some(ref l) = self.listener {
            if let Ok((stream, addr)) = l.accept() {
                return JSocket {
                    stream: Some(stream),
                    host: addr.ip().to_string(),
                    port: addr.port() as i32,
                };
            }
        }
        JSocket::default()
    }

    /// `ServerSocket.getLocalPort()`
    #[allow(non_snake_case)]
    pub fn getLocalPort(&self) -> i32 {
        self.listener
            .as_ref()
            .and_then(|l| l.local_addr().ok())
            .map(|a| a.port() as i32)
            .unwrap_or(-1)
    }

    /// `ServerSocket.isClosed()`
    #[allow(non_snake_case)]
    pub fn isClosed(&self) -> bool {
        self.listener.is_none()
    }

    /// `ServerSocket.close()`
    pub fn close(&mut self) {
        self.listener = None;
    }
}

// ─── HttpURLConnection ───────────────────────────────────────────────────────

/// Rust equivalent of `java.net.HttpURLConnection`.
///
/// A minimal HTTP/1.1 client that uses raw `TcpStream`.  Supports GET and POST.
#[derive(Debug, Clone, Default)]
pub struct JHttpURLConnection {
    url: JURL,
    method: String,
    headers: Vec<(String, String)>,
    response_code: i32,
    response_body: String,
    connected: bool,
}

impl JHttpURLConnection {
    /// Created via `url.openConnection()`.
    pub fn from_url(url: &JURL) -> Self {
        JHttpURLConnection {
            url: url.clone(),
            method: "GET".to_string(),
            headers: Vec::new(),
            response_code: -1,
            response_body: String::new(),
            connected: false,
        }
    }

    /// `HttpURLConnection.setRequestMethod(String)`
    #[allow(non_snake_case)]
    pub fn setRequestMethod(&mut self, method: JString) {
        self.method = method.as_str().to_string();
    }

    /// `HttpURLConnection.setRequestProperty(String, String)`
    #[allow(non_snake_case)]
    pub fn setRequestProperty(&mut self, key: JString, value: JString) {
        self.headers
            .push((key.as_str().to_string(), value.as_str().to_string()));
    }

    /// `HttpURLConnection.getResponseCode()`
    #[allow(non_snake_case)]
    pub fn getResponseCode(&mut self) -> i32 {
        if !self.connected {
            self.connect();
        }
        self.response_code
    }

    /// `HttpURLConnection.getResponseMessage()`
    #[allow(non_snake_case)]
    pub fn getResponseMessage(&mut self) -> JString {
        if !self.connected {
            self.connect();
        }
        JString::from(match self.response_code {
            200 => "OK",
            201 => "Created",
            301 => "Moved Permanently",
            302 => "Found",
            400 => "Bad Request",
            401 => "Unauthorized",
            403 => "Forbidden",
            404 => "Not Found",
            500 => "Internal Server Error",
            _ => "Unknown",
        })
    }

    /// `HttpURLConnection.getContentLength()`
    #[allow(non_snake_case)]
    pub fn getContentLength(&mut self) -> i32 {
        if !self.connected {
            self.connect();
        }
        self.response_body.len() as i32
    }

    /// Read the response body as a string (for use with BufferedReader/Scanner).
    #[allow(non_snake_case)]
    pub fn getResponseBody(&mut self) -> JString {
        if !self.connected {
            self.connect();
        }
        JString::from(self.response_body.as_str())
    }

    /// `HttpURLConnection.disconnect()`
    pub fn disconnect(&mut self) {
        self.connected = false;
    }

    fn connect(&mut self) {
        self.connected = true;

        let host = self.url.getHost();
        let port = self.url.getPort();
        let port = if port == -1 {
            self.url.getDefaultPort()
        } else {
            port
        };
        if port <= 0 {
            self.response_code = -1;
            return;
        }

        let addr = format!("{}:{}", host.as_str(), port);
        let stream = match TcpStream::connect(&addr) {
            Ok(s) => s,
            Err(_) => {
                self.response_code = -1;
                return;
            }
        };

        let path = self.url.getFile();
        let path_str = if path.as_str().is_empty() {
            "/"
        } else {
            path.as_str()
        };

        let mut request = format!("{} {} HTTP/1.1\r\n", self.method, path_str);
        request.push_str(&format!("Host: {}\r\n", host.as_str()));
        // Add user headers
        for (k, v) in &self.headers {
            request.push_str(&format!("{k}: {v}\r\n"));
        }
        request.push_str("Connection: close\r\n");
        request.push_str("\r\n");

        let mut stream = stream;
        if stream.write_all(request.as_bytes()).is_err() {
            self.response_code = -1;
            return;
        }
        let _ = stream.flush();

        let mut response = String::new();
        if stream.read_to_string(&mut response).is_err() {
            self.response_code = -1;
            return;
        }

        // Parse status line
        if let Some(status_line) = response.lines().next() {
            let parts: Vec<&str> = status_line.splitn(3, ' ').collect();
            if parts.len() >= 2 {
                self.response_code = parts[1].parse().unwrap_or(-1);
            }
        }

        // Parse body (after \r\n\r\n)
        if let Some(body_start) = response.find("\r\n\r\n") {
            self.response_body = response[body_start + 4..].to_string();
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_parse_components() {
        let u = JURL::new(JString::from("http://example.com:8080/path?q=1#frag"));
        assert_eq!(u.getProtocol().as_str(), "http");
        assert_eq!(u.getHost().as_str(), "example.com");
        assert_eq!(u.getPort(), 8080);
        assert_eq!(u.getPath().as_str(), "/path");
        assert_eq!(u.getQuery().as_str(), "q=1");
        assert_eq!(u.getRef().as_str(), "frag");
    }

    #[test]
    fn url_no_port() {
        let u = JURL::new(JString::from("https://example.com/index.html"));
        assert_eq!(u.getProtocol().as_str(), "https");
        assert_eq!(u.getHost().as_str(), "example.com");
        assert_eq!(u.getPort(), -1);
        assert_eq!(u.getDefaultPort(), 443);
        assert_eq!(u.getPath().as_str(), "/index.html");
    }

    #[test]
    fn url_display() {
        let u = JURL::new(JString::from("http://localhost/test"));
        assert_eq!(format!("{u}"), "http://localhost/test");
    }

    #[test]
    fn socket_default() {
        let s = JSocket::default();
        assert!(s.isClosed());
        assert!(!s.isConnected());
    }

    #[test]
    fn server_socket_default() {
        let s = JServerSocket::default();
        assert!(s.isClosed());
    }

    #[test]
    fn http_connection_default() {
        let u = JURL::new(JString::from("http://example.com"));
        let conn = JHttpURLConnection::from_url(&u);
        assert!(!conn.connected);
    }
}

#![allow(non_snake_case)]
//! Java Process/ProcessBuilder runtime types.
//!
//! Provides Rust equivalents for:
//! - `java.lang.ProcessBuilder` → [`JProcessBuilder`]
//! - `java.lang.Process` → [`JProcess`]

use crate::io::{JBufferedReader, JFile};
use crate::string::JString;
use std::path::PathBuf;
use std::process::{Command, Stdio};

// ─── JProcessBuilder ──────────────────────────────────────────────────────────

/// Java `java.lang.ProcessBuilder` — builds and starts subprocesses.
///
/// Wraps [`std::process::Command`].  Method names use Java's camelCase
/// convention.
#[derive(Debug, Clone)]
pub struct JProcessBuilder {
    command: Vec<String>,
    directory: Option<PathBuf>,
    env_extra: Vec<(String, String)>,
    redirect_error_stream: bool,
}

impl JProcessBuilder {
    /// Java `new ProcessBuilder(String... command)` — varargs constructor.
    pub fn new_varargs(args: Vec<JString>) -> Self {
        Self {
            command: args.into_iter().map(|s| s.to_string()).collect(),
            directory: None,
            env_extra: Vec::new(),
            redirect_error_stream: false,
        }
    }

    /// Java `new ProcessBuilder(List<String> command)` — list constructor.
    pub fn new_list(args: crate::list::JList<JString>) -> Self {
        Self {
            command: args.iter().map(|s| s.to_string()).collect(),
            directory: None,
            env_extra: Vec::new(),
            redirect_error_stream: false,
        }
    }

    /// Java `pb.command(newCommand)` — replace the command list, returns self.
    pub fn command(&mut self, args: crate::list::JList<JString>) -> JProcessBuilder {
        self.command = args.iter().map(|s| s.to_string()).collect();
        self.clone()
    }

    /// Java `pb.directory(file)` — set working directory, returns self.
    pub fn directory(&mut self, dir: JFile) -> JProcessBuilder {
        self.directory = Some(dir.path_buf().clone());
        self.clone()
    }

    /// Java `pb.redirectErrorStream(bool)` — merge stderr into stdout, returns self.
    pub fn redirectErrorStream(&mut self, b: bool) -> JProcessBuilder {
        self.redirect_error_stream = b;
        self.clone()
    }

    /// Java `pb.inheritIO()` — inherit parent's stdio streams, returns self.
    ///
    /// In this implementation stdout is always piped so output can be
    /// captured via [`JProcess::getInputStream`].
    pub fn inheritIO(&mut self) -> JProcessBuilder {
        self.clone()
    }

    /// Java `pb.start()` — spawn the process and return a [`JProcess`].
    ///
    /// # Panics
    /// Panics if the command cannot be found or the OS rejects the spawn.
    pub fn start(&mut self) -> JProcess {
        assert!(
            !self.command.is_empty(),
            "ProcessBuilder: command list is empty"
        );
        let mut cmd = Command::new(&self.command[0]);
        if self.command.len() > 1 {
            cmd.args(&self.command[1..]);
        }
        if let Some(ref dir) = self.directory {
            cmd.current_dir(dir);
        }
        for (k, v) in &self.env_extra {
            cmd.env(k, v);
        }
        cmd.stdout(Stdio::piped());
        if self.redirect_error_stream {
            // Discard stderr when merging (full fd-dup not available in std).
            cmd.stderr(Stdio::null());
        } else {
            cmd.stderr(Stdio::piped());
        }
        let child = cmd.spawn().unwrap_or_else(|e| {
            panic!("ProcessBuilder.start() failed to spawn {:?}: {}", self.command, e)
        });
        JProcess::new(child)
    }

    // ── Convenience: Runtime.getRuntime().exec(String) ────────────────────

    /// Java `Runtime.getRuntime().exec(command)` — split command on whitespace
    /// and start immediately.
    pub fn exec_string(cmd: JString) -> JProcess {
        let parts: Vec<JString> = cmd
            .as_str()
            .split_whitespace()
            .map(JString::from)
            .collect();
        JProcessBuilder::new_varargs(parts).start()
    }

    /// Java `Runtime.getRuntime().exec(String[] command)` — start from array.
    pub fn exec_array(cmd: crate::array::JArray<JString>) -> JProcess {
        let parts: Vec<JString> = (0..cmd.length()).map(|i| cmd.get(i)).collect();
        JProcessBuilder::new_varargs(parts).start()
    }
}

// ─── JProcess ─────────────────────────────────────────────────────────────────

/// Java `java.lang.Process` — a running subprocess.
///
/// Created by [`JProcessBuilder::start`].  stdout/stderr are piped and
/// exposed as [`JBufferedReader`] via [`getInputStream`](JProcess::getInputStream) /
/// [`getErrorStream`](JProcess::getErrorStream).
pub struct JProcess {
    child: std::process::Child,
    exit_code: Option<i32>,
}

impl std::fmt::Debug for JProcess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JProcess")
            .field("exit_code", &self.exit_code)
            .finish()
    }
}

impl JProcess {
    fn new(child: std::process::Child) -> Self {
        Self {
            child,
            exit_code: None,
        }
    }

    /// Java `p.getInputStream()` — returns stdout as a [`JBufferedReader`].
    ///
    /// Takes the stdout pipe from the child process.  Subsequent calls on the
    /// same process return an empty reader.
    pub fn getInputStream(&mut self) -> JBufferedReader {
        if let Some(stdout) = self.child.stdout.take() {
            JBufferedReader::from_bufreader(std::io::BufReader::new(stdout))
        } else {
            JBufferedReader::from_raw_string(String::new())
        }
    }

    /// Java `p.getErrorStream()` — returns stderr as a [`JBufferedReader`].
    pub fn getErrorStream(&mut self) -> JBufferedReader {
        if let Some(stderr) = self.child.stderr.take() {
            JBufferedReader::from_bufreader(std::io::BufReader::new(stderr))
        } else {
            JBufferedReader::from_raw_string(String::new())
        }
    }

    /// Java `p.waitFor()` — block until the process exits and return the exit
    /// code.
    pub fn waitFor(&mut self) -> i32 {
        if let Some(code) = self.exit_code {
            return code;
        }
        let status = self
            .child
            .wait()
            .expect("JProcess.waitFor(): wait() failed");
        let code = status.code().unwrap_or(0);
        self.exit_code = Some(code);
        code
    }

    /// Java `p.exitValue()` — return the cached exit code.
    ///
    /// Only meaningful after [`waitFor`](JProcess::waitFor) has been called.
    pub fn exitValue(&self) -> i32 {
        self.exit_code.unwrap_or(0)
    }

    /// Java `p.destroy()` — kill the subprocess.
    pub fn destroy(&mut self) {
        let _ = self.child.kill();
    }

    /// Java `p.isAlive()` — check if the subprocess is still running.
    pub fn isAlive(&mut self) -> bool {
        self.child
            .try_wait()
            .map(|status| status.is_none())
            .unwrap_or(false)
    }
}

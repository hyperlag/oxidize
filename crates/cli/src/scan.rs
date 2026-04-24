//! Java project compatibility scanner.
//!
//! Analyses Java source files for patterns that `jtrans` does not yet support,
//! providing a pre-flight report before you attempt a full translation.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use regex::Regex;

// ── Public types ────────────────────────────────────────────────────────

/// How serious an issue is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// May cause incorrect behaviour or compile errors in edge cases.
    Warning,
    /// Will cause a parse / codegen failure; file cannot be translated as-is.
    Error,
}

/// A single compatibility issue found in a Java source file.
#[derive(Debug, Clone)]
pub struct ScanIssue {
    /// 1-based source line number, if known.
    pub line: Option<usize>,
    pub severity: Severity,
    /// Short machine-readable code (e.g. `"reflection"`, `"native-method"`).
    pub code: &'static str,
    /// Human-readable description.
    pub message: String,
}

/// All issues found in one file.
#[derive(Debug)]
pub struct FileScanResult {
    pub path: PathBuf,
    pub issues: Vec<ScanIssue>,
}

impl FileScanResult {
    pub fn is_ok(&self) -> bool {
        self.issues.is_empty()
    }
    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .count()
    }
    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .count()
    }
}

/// Aggregated report for an entire project scan.
#[derive(Debug)]
pub struct ScanReport {
    pub results: Vec<FileScanResult>,
}

impl ScanReport {
    pub fn files_ok(&self) -> usize {
        self.results.iter().filter(|r| r.is_ok()).count()
    }
    pub fn files_with_errors(&self) -> usize {
        self.results.iter().filter(|r| r.error_count() > 0).count()
    }
    pub fn files_with_warnings_only(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.error_count() == 0 && r.warning_count() > 0)
            .count()
    }
    pub fn total_errors(&self) -> usize {
        self.results.iter().map(|r| r.error_count()).sum()
    }
    pub fn total_warnings(&self) -> usize {
        self.results.iter().map(|r| r.warning_count()).sum()
    }
    pub fn has_errors(&self) -> bool {
        self.total_errors() > 0
    }
}

// ── Pattern-based checks ────────────────────────────────────────────────

/// A single line-level pattern check.
struct Check {
    severity: Severity,
    code: &'static str,
    description: &'static str,
    /// Regex applied to each line of source (after stripping line comments).
    pattern: &'static str,
}

/// All checks, compiled once on first use.
static CHECKS: &[Check] = &[
    // ── Hard errors ─────────────────────────────────────────────────
    Check {
        severity: Severity::Error,
        code: "native-method",
        description: "native method declaration",
        pattern: r"\bnative\b",
    },
    Check {
        severity: Severity::Error,
        code: "reflection-import",
        description: "java.lang.reflect import (reflection not supported)",
        pattern: r"\bimport\s+java\.lang\.reflect\b",
    },
    Check {
        severity: Severity::Error,
        code: "reflection-class-for-name",
        description: "Class.forName() — dynamic class loading not supported",
        pattern: r"\bClass\.forName\s*\(",
    },
    Check {
        severity: Severity::Error,
        code: "reflection-method-invoke",
        description: "Method.invoke() — reflective dispatch not supported",
        pattern: r"\bMethod\b.*\.invoke\s*\(",
    },
    Check {
        severity: Severity::Error,
        code: "reflection-field-access",
        description: "Field.get()/set() — reflective field access not supported",
        pattern: r"\bField\b.*\.(get|set)\s*\(",
    },
    Check {
        severity: Severity::Error,
        code: "reflection-constructor",
        description: "Constructor.newInstance() — reflective construction not supported",
        pattern: r"\bConstructor\b.*\.newInstance\s*\(",
    },
    Check {
        severity: Severity::Error,
        code: "reflection-declared-members",
        description: "getDeclaredMethods/Fields/Constructors — runtime reflection not supported",
        pattern: r"\bgetDeclared(Methods|Fields|Constructors|Method|Field|Constructor)\s*\(",
    },
    Check {
        severity: Severity::Error,
        code: "reflection-set-accessible",
        description: "setAccessible() — bypassing access modifiers not supported",
        pattern: r"\bsetAccessible\s*\(",
    },
    Check {
        severity: Severity::Error,
        code: "classloader",
        description: "ClassLoader usage — dynamic class loading not supported",
        pattern: r"\bClassLoader\b",
    },
    Check {
        severity: Severity::Error,
        code: "load-library",
        description: "System.loadLibrary() / System.load() — JNI not supported",
        pattern: r"\bSystem\s*\.\s*(loadLibrary|load)\s*\(",
    },
    Check {
        severity: Severity::Error,
        code: "annotation-processing-import",
        description: "javax.annotation.processing import — annotation processors not supported",
        pattern: r"\bimport\s+javax\.annotation\.processing\b",
    },
    Check {
        severity: Severity::Error,
        code: "rmi-import",
        description: "java.rmi import — Remote Method Invocation not supported",
        pattern: r"\bimport\s+java\.rmi\b",
    },
    Check {
        severity: Severity::Error,
        code: "nio-channels-import",
        description: "java.nio.channels import — NIO selectors/channels not supported",
        pattern: r"\bimport\s+java\.nio\.channels\b",
    },
    Check {
        severity: Severity::Error,
        code: "object-streams",
        description: "ObjectInputStream/ObjectOutputStream — Java serialization not supported",
        pattern: r"\bObject(Input|Output)Stream\b",
    },
    Check {
        severity: Severity::Error,
        code: "instrument-import",
        description: "java.lang.instrument import — bytecode instrumentation not supported",
        pattern: r"\bimport\s+java\.lang\.instrument\b",
    },
    Check {
        severity: Severity::Error,
        code: "colon-form-pattern-switch",
        description:
            "colon-form type-pattern switch label (`case Type var:`) — use arrow-form instead",
        // Matches `case <UpperType> <lowerVar>:` but not `case CONSTANT:` or `case "str":`
        pattern: r"\bcase\s+[A-Z][A-Za-z0-9_]*(?:<[^>]*>)?\s+[a-z_][A-Za-z0-9_]*\s*:",
    },
    Check {
        severity: Severity::Error,
        code: "dynamic-proxy",
        description: "Proxy.newProxyInstance() — dynamic proxy not supported",
        pattern: r"\bProxy\.newProxyInstance\s*\(",
    },
    // ── Warnings ─────────────────────────────────────────────────────
    Check {
        severity: Severity::Warning,
        code: "serializable",
        description: "implements Serializable — parsing succeeds but serialization has no effect",
        pattern: r"\bimplements\b[^{]*\bSerializable\b",
    },
    Check {
        severity: Severity::Warning,
        code: "spring-annotations",
        description: "Spring/JPA annotation — framework injection will NOT work after translation",
        pattern: r"@(Autowired|Component|Service|Repository|Controller|RestController|SpringBootApplication|Entity|Table|Column|Transactional|RequestMapping|GetMapping|PostMapping|PutMapping|DeleteMapping|Bean|Configuration|EnableAutoConfiguration)\b",
    },
    Check {
        severity: Severity::Warning,
        code: "runtime-getruntime",
        description: "Runtime.getRuntime() — only .exec() is supported; other methods will fail",
        pattern: r"\bRuntime\.getRuntime\(\)\s*\.",
    },
    Check {
        severity: Severity::Warning,
        code: "externalizable",
        description: "implements Externalizable — not supported (treated like Serializable)",
        pattern: r"\bimplements\b[^{]*\bExternalizable\b",
    },
];

// ── Scanner ─────────────────────────────────────────────────────────────

/// Scan a list of Java source files and return a full report.
pub fn scan_files(files: &[PathBuf]) -> ScanReport {
    // Compile all regexes once.
    let compiled: Vec<(&Check, Regex)> = CHECKS
        .iter()
        .map(|c| (c, Regex::new(c.pattern).expect("built-in pattern is valid")))
        .collect();

    let results = files.iter().map(|path| scan_one(path, &compiled)).collect();

    ScanReport { results }
}

fn scan_one(path: &Path, compiled: &[(&Check, Regex)]) -> FileScanResult {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            return FileScanResult {
                path: path.to_path_buf(),
                issues: vec![ScanIssue {
                    line: None,
                    severity: Severity::Error,
                    code: "io-error",
                    message: format!("cannot read file: {e}"),
                }],
            };
        }
    };

    // Special single-file check: module-info.java is unsupported entirely.
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let mut issues = Vec::new();

    if filename == "module-info.java" {
        issues.push(ScanIssue {
            line: None,
            severity: Severity::Error,
            code: "module-info",
            message: "module-info.java (Java 9+ module declarations) is not supported".to_string(),
        });
    }

    // Line-by-line pattern scan.
    for (line_num, line) in source.lines().enumerate() {
        let line_num = line_num + 1; // 1-based
        let stripped = strip_line_comment(line);
        for (check, re) in compiled {
            if re.is_match(stripped) {
                issues.push(ScanIssue {
                    line: Some(line_num),
                    severity: check.severity,
                    code: check.code,
                    message: check.description.to_string(),
                });
            }
        }
    }

    // Parser-based check — attempt a full parse; report any parse errors.
    match parser::parse_to_ir(&source) {
        Ok(_) => {}
        Err(e) => {
            // Try to figure out a line number from the error message.
            let line = extract_line_number_from_error(&format!("{e:#}"));
            issues.push(ScanIssue {
                line,
                severity: Severity::Error,
                code: "parse-error",
                message: format!("parser rejected this file: {e:#}"),
            });
        }
    }

    // De-duplicate: if a pattern check fires AND the parser also rejects for the
    // same reason, the parser error is more authoritative — keep both but avoid
    // pure duplicates from identical lines.
    issues.dedup_by(|a, b| a.code == b.code && a.line == b.line && a.message == b.message);

    issues.sort_by_key(|i| (i.line.unwrap_or(0), i.severity));

    FileScanResult {
        path: path.to_path_buf(),
        issues,
    }
}

/// Strip the `//` line comment portion from a source line.
/// Does not handle `/* */` block comments — good enough for pattern matching.
fn strip_line_comment(line: &str) -> &str {
    // Walk characters, skip anything inside a string literal, stop at `//`.
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut in_string = false;
    while i < bytes.len() {
        match bytes[i] {
            b'"' if !in_string => in_string = true,
            b'"' if in_string => in_string = false,
            b'\\' if in_string => i += 1, // skip escaped char
            b'/' if !in_string && i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                return &line[..i];
            }
            _ => {}
        }
        i += 1;
    }
    line
}

/// Attempt a crude heuristic extraction of a line number from a parser error
/// message such as `"parse error at 1:42"` or `"row 10, col 3"`.
fn extract_line_number_from_error(msg: &str) -> Option<usize> {
    // Try patterns like " at 1:42" or "line 1" or "row 1,"
    let patterns = [
        regex::Regex::new(r"\bat\s+(\d+):\d+").unwrap(),
        regex::Regex::new(r"(?i)line\s+(\d+)").unwrap(),
        regex::Regex::new(r"(?i)row\s+(\d+)").unwrap(),
    ];
    for re in &patterns {
        if let Some(cap) = re.captures(msg) {
            if let Ok(n) = cap[1].parse::<usize>() {
                return Some(n);
            }
        }
    }
    None
}

// ── Report printing ──────────────────────────────────────────────────────

/// Print a human-readable report to stdout.
/// Returns `true` if any errors were found.
pub fn print_report(report: &ScanReport, issues_only: bool) -> bool {
    let total = report.results.len();
    println!(
        "\nScanning {} Java file{}…\n",
        total,
        if total == 1 { "" } else { "s" }
    );

    for result in &report.results {
        let label = result
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("<unknown>");

        // Keep full path context but anchor on filename for compact display.
        let display_path = result.path.display().to_string();

        if result.is_ok() {
            if !issues_only {
                println!("  ✓  {display_path}");
            }
            continue;
        }

        let err_count = result.error_count();
        let warn_count = result.warning_count();
        let mut summary = String::new();
        if err_count > 0 {
            let _ = write!(
                summary,
                "{err_count} error{}",
                if err_count == 1 { "" } else { "s" }
            );
        }
        if warn_count > 0 {
            if !summary.is_empty() {
                summary.push_str(", ");
            }
            let _ = write!(
                summary,
                "{warn_count} warning{}",
                if warn_count == 1 { "" } else { "s" }
            );
        }

        let marker = if err_count > 0 { "✗" } else { "⚠" };
        println!("  {marker}  {display_path}  [{summary}]");

        for issue in &result.issues {
            let sev = match issue.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
            };
            let loc = issue
                .line
                .map(|l| format!("line {l}: "))
                .unwrap_or_default();
            println!("       {loc}[{sev}:{}] {}", issue.code, issue.message);
        }
        println!();

        let _ = label; // used in display_path already
    }

    // ── Summary ──────────────────────────────────────────────────────
    let bar = "═".repeat(50);
    println!("{bar}");
    println!("Summary");
    println!("{bar}");
    println!("  Files scanned            : {}", report.results.len());
    println!("  Files fully compatible   : {}", report.files_ok());
    if report.files_with_warnings_only() > 0 {
        println!(
            "  Files with warnings only : {}",
            report.files_with_warnings_only()
        );
    }
    println!(
        "  Files with errors        : {}",
        report.files_with_errors()
    );
    if report.total_errors() > 0 || report.total_warnings() > 0 {
        println!();
        if report.total_errors() > 0 {
            println!("  Total errors             : {}", report.total_errors());
        }
        if report.total_warnings() > 0 {
            println!("  Total warnings           : {}", report.total_warnings());
        }
    }

    // Issue-code breakdown
    if report.has_errors() || report.total_warnings() > 0 {
        let mut code_counts: std::collections::BTreeMap<(&str, Severity), usize> =
            std::collections::BTreeMap::new();
        for result in &report.results {
            for issue in &result.issues {
                *code_counts.entry((issue.code, issue.severity)).or_insert(0) += 1;
            }
        }

        println!();
        println!("  Issue breakdown:");
        for ((code, sev), count) in &code_counts {
            let sev_str = match sev {
                Severity::Error => "error",
                Severity::Warning => "warning",
            };
            println!("    {count:>4}×  [{sev_str}:{code}]");
        }
    }

    println!();

    if !report.has_errors() {
        if report.total_warnings() == 0 {
            println!("All files look compatible — ready to translate!");
        } else {
            println!("No blocking errors found. Review warnings before translating.");
        }
    } else {
        println!(
            "{} file(s) have blocking errors that must be addressed before translation.",
            report.files_with_errors()
        );
    }
    println!();

    report.has_errors()
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn issues_for(source: &str) -> Vec<ScanIssue> {
        let compiled: Vec<(&Check, Regex)> = CHECKS
            .iter()
            .map(|c| (c, Regex::new(c.pattern).expect("valid")))
            .collect();

        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), source).unwrap();
        let result = scan_one(tmp.path(), &compiled);
        result.issues
    }

    fn has_code(issues: &[ScanIssue], code: &str) -> bool {
        issues.iter().any(|i| i.code == code)
    }

    #[test]
    fn detects_native_method() {
        let src = "public class Foo { public native void doThing(); }";
        assert!(has_code(&issues_for(src), "native-method"));
    }

    #[test]
    fn detects_reflection_import() {
        let src = "import java.lang.reflect.Method;\npublic class Foo {}";
        assert!(has_code(&issues_for(src), "reflection-import"));
    }

    #[test]
    fn detects_class_for_name() {
        let src = "public class Foo { void f() { Class.forName(\"Foo\"); } }";
        assert!(has_code(&issues_for(src), "reflection-class-for-name"));
    }

    #[test]
    fn detects_spring_annotation() {
        let src = "@Service\npublic class Foo {}";
        assert!(has_code(&issues_for(src), "spring-annotations"));
    }

    #[test]
    fn detects_colon_form_pattern_switch() {
        let src = r"public class Foo {
  void f(Object o) {
    switch (o) {
      case String s:
        break;
    }
  }
}";
        assert!(has_code(&issues_for(src), "colon-form-pattern-switch"));
    }

    #[test]
    fn no_false_positive_on_enum_constant_switch() {
        // `case ACTIVE:` — enum constant, not a type pattern
        let src = r"public class Foo {
  enum State { ACTIVE, INACTIVE }
  void f(State s) {
    switch (s) {
      case ACTIVE: break;
    }
  }
}";
        // Should not trigger colon-form-pattern-switch
        let issues = issues_for(src);
        assert!(
            !has_code(&issues, "colon-form-pattern-switch"),
            "false positive: {:?}",
            issues
        );
    }

    #[test]
    fn no_false_positive_on_string_switch() {
        let src = r#"public class Foo {
  void f(String s) {
    switch (s) {
      case "hello": break;
    }
  }
}"#;
        let issues = issues_for(src);
        assert!(
            !has_code(&issues, "colon-form-pattern-switch"),
            "false positive: {:?}",
            issues
        );
    }

    #[test]
    fn clean_file_has_no_issues() {
        let src = r#"public class HelloWorld {
  public static void main(String[] args) {
    System.out.println("Hello, World!");
  }
}"#;
        assert!(issues_for(src).is_empty());
    }

    #[test]
    fn strip_line_comment_removes_trailing_comment() {
        assert_eq!(
            strip_line_comment("  int x = 1; // native"),
            "  int x = 1; "
        );
    }

    #[test]
    fn strip_line_comment_preserves_string_with_slashes() {
        let line = r#"  String s = "http://example.com"; // end"#;
        assert_eq!(
            strip_line_comment(line),
            r#"  String s = "http://example.com"; "#
        );
    }
}

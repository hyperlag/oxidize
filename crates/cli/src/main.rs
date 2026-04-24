//! `jtrans` — command-line driver for the Java→Rust translator.

mod cache;
mod scan;
mod source_map;

use anyhow::Context;
use clap::{ArgAction, Parser, Subcommand};
use std::path::{Path, PathBuf};
use tracing::{error, info, warn};

use cache::TranslationCache;
use source_map::SourceMap;

#[derive(Parser, Debug)]
#[command(
    name = "jtrans",
    version,
    about = "Translate Java source files to idiomatic, memory-safe Rust"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    // ── Legacy positional interface (backwards-compat) ──────────────
    /// Java source file(s) to translate (legacy positional mode).
    #[arg(global = false)]
    inputs: Vec<PathBuf>,

    /// Output directory for the generated Rust source files.
    #[arg(short, long, default_value = "out")]
    output: PathBuf,

    /// Print the IR as JSON after parsing (debugging aid).
    #[arg(long)]
    dump_ir: bool,

    /// Print the generated Rust to stdout instead of writing to disk.
    #[arg(long)]
    print: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Translate Java source files to Rust.
    Translate {
        /// Input directory or file(s) containing Java source code.
        #[arg(short, long, required = true)]
        input: Vec<PathBuf>,

        /// Output directory for the generated Rust project.
        #[arg(short, long, default_value = "rust-out")]
        output: PathBuf,

        /// Classpath entries (directories or JARs) for type resolution.
        #[arg(short, long)]
        classpath: Vec<PathBuf>,

        /// Watch input files and re-translate on changes.
        #[arg(long)]
        watch: bool,

        /// Print the IR as JSON after parsing (debugging aid).
        #[arg(long)]
        dump_ir: bool,

        /// Print the generated Rust to stdout instead of writing to disk.
        #[arg(long)]
        print: bool,

        /// Skip files that have not changed since the last translation.
        #[arg(long, default_value_t = true, action = ArgAction::Set)]
        #[arg(long = "no-incremental", action = ArgAction::SetFalse, overrides_with = "incremental")]
        incremental: bool,

        /// Generate source map files (.jtrans-map) alongside output.
        #[arg(long, default_value_t = true, action = ArgAction::Set)]
        #[arg(long = "no-source-map", action = ArgAction::SetFalse, overrides_with = "source_map")]
        source_map: bool,

        /// Generate a Cargo.toml in the output directory.
        #[arg(long, default_value_t = true, action = ArgAction::Set)]
        #[arg(long = "no-cargo-toml", action = ArgAction::SetFalse, overrides_with = "cargo_toml")]
        cargo_toml: bool,
    },

    /// Initialize a Maven-compatible pom.xml fragment for jtrans integration.
    InitMaven {
        /// Directory to write the pom.xml fragment to.
        #[arg(short, long, default_value = ".")]
        output: PathBuf,
    },

    /// Initialize a Gradle-compatible build script fragment for jtrans integration.
    InitGradle {
        /// Directory to write the build.gradle.kts fragment to.
        #[arg(short, long, default_value = ".")]
        output: PathBuf,
    },

    /// Scan a Java project for compatibility issues before translation.
    ///
    /// Analyses each .java file for patterns that jtrans does not support
    /// (reflection, native methods, Spring annotations, etc.) and reports
    /// a per-file breakdown plus a project-level summary.
    Scan {
        /// Input directory or file(s) to scan.
        #[arg(short, long, required = true)]
        input: Vec<PathBuf>,

        /// Only print files that have issues (suppress the ✓ lines).
        #[arg(long)]
        issues_only: bool,

        /// Exit with a non-zero status code if any blocking errors are found.
        #[arg(long)]
        strict: bool,
    },
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("jtrans=info".parse().unwrap()),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Translate {
            input,
            output,
            classpath,
            watch,
            dump_ir,
            print,
            incremental,
            source_map,
            cargo_toml,
        }) => {
            for cp in &classpath {
                info!("classpath entry: {cp:?}");
            }

            let mut java_files = collect_java_files(&input)?;

            // Also collect .java sources from classpath directories.
            let cp_files = collect_java_files(&classpath).unwrap_or_default();
            if !cp_files.is_empty() {
                info!("{} Java file(s) found on classpath", cp_files.len());
                java_files.extend(cp_files);
            }

            if java_files.is_empty() {
                anyhow::bail!("No .java files found in the specified input paths");
            }

            translate_files(
                &java_files,
                &output,
                dump_ir,
                print,
                incremental,
                source_map,
                cargo_toml,
            )?;

            if watch {
                run_watch_mode(
                    &input,
                    &classpath,
                    &output,
                    dump_ir,
                    print,
                    incremental,
                    source_map,
                    cargo_toml,
                )?;
            }
        }

        Some(Commands::InitMaven { output }) => {
            write_maven_fragment(&output)?;
        }

        Some(Commands::InitGradle { output }) => {
            write_gradle_fragment(&output)?;
        }

        Some(Commands::Scan {
            input,
            issues_only,
            strict,
        }) => {
            let java_files = collect_java_files(&input)?;
            if java_files.is_empty() {
                anyhow::bail!("No .java files found in the specified input paths");
            }
            info!("scanning {} Java file(s)…", java_files.len());
            let report = scan::scan_files(&java_files);
            let had_errors = scan::print_report(&report, issues_only);
            if strict && had_errors {
                std::process::exit(1);
            }
        }

        None => {
            // Legacy positional mode for backwards compatibility.
            if cli.inputs.is_empty() {
                anyhow::bail!(
                    "No inputs specified. Use `jtrans translate --input <path>` or provide positional arguments."
                );
            }
            translate_files_legacy(&cli.inputs, &cli.output, cli.dump_ir, cli.print)?;
        }
    }

    Ok(())
}

// ── Collect Java files from input paths ─────────────────────────────────

fn collect_java_files(inputs: &[PathBuf]) -> anyhow::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for input in inputs {
        if input.is_file() {
            if input.extension().and_then(|e| e.to_str()) == Some("java") {
                files.push(input.clone());
            } else {
                warn!("Skipping non-Java file: {input:?}");
            }
        } else if input.is_dir() {
            collect_java_files_recursive(input, &mut files)?;
        } else {
            anyhow::bail!("Input path does not exist: {input:?}");
        }
    }
    Ok(files)
}

fn collect_java_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading directory {dir:?}"))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_java_files_recursive(&path, files)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("java") {
            files.push(path);
        }
    }
    Ok(())
}

// ── Core translation pipeline ───────────────────────────────────────────

fn translate_one(
    input: &Path,
    output_dir: &Path,
    dump_ir: bool,
    print: bool,
    gen_source_map: bool,
) -> anyhow::Result<()> {
    let source = std::fs::read_to_string(input).with_context(|| format!("reading {input:?}"))?;

    info!("parsing {input:?}");
    let ir_module = parser::parse_to_ir(&source).with_context(|| format!("parsing {input:?}"))?;

    if dump_ir {
        let json = serde_json::to_string_pretty(&ir_module).context("serialising IR to JSON")?;
        println!("{json}");
    }

    info!("type-checking {input:?}");
    let typed_module =
        typeck::type_check(ir_module).with_context(|| format!("type-checking {input:?}"))?;

    info!("generating Rust for {input:?}");
    let rust_src =
        codegen::generate(&typed_module).with_context(|| format!("code-generating {input:?}"))?;

    if print {
        println!("{rust_src}");
    } else {
        std::fs::create_dir_all(output_dir)
            .with_context(|| format!("creating output dir {output_dir:?}"))?;

        let stem = input
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        let out_path = output_dir.join(format!("{}.rs", stem.to_lowercase()));
        std::fs::write(&out_path, &rust_src).with_context(|| format!("writing {out_path:?}"))?;
        info!("wrote {out_path:?}");

        // Generate source map alongside the Rust file.
        if gen_source_map {
            let smap = SourceMap::build(&source, &rust_src);
            let map_path = output_dir.join(format!("{}.jtrans-map", stem.to_lowercase()));
            std::fs::write(&map_path, smap.to_string())
                .with_context(|| format!("writing source map {map_path:?}"))?;
            info!("wrote source map {map_path:?}");
        }
    }

    Ok(())
}

fn translate_files(
    java_files: &[PathBuf],
    output: &Path,
    dump_ir: bool,
    print: bool,
    incremental: bool,
    gen_source_map: bool,
    cargo_toml: bool,
) -> anyhow::Result<()> {
    let src_dir = if print {
        output.to_path_buf()
    } else {
        output.join("src")
    };

    let mut cache = if incremental {
        TranslationCache::load(output)
    } else {
        TranslationCache::new()
    };

    let mut translated = 0usize;
    let mut skipped = 0usize;

    let mut had_errors = false;

    for java_file in java_files {
        if incremental && !cache.is_stale(java_file)? {
            info!("skipping (unchanged): {java_file:?}");
            skipped += 1;
            continue;
        }

        match translate_one(java_file, &src_dir, dump_ir, print, gen_source_map) {
            Ok(()) => {
                if incremental {
                    cache.update(java_file)?;
                }
                translated += 1;
            }
            Err(e) => {
                error!("failed to translate {java_file:?}: {e:#}");
                had_errors = true;
            }
        }
    }

    if incremental {
        cache.save(output)?;
    }

    if cargo_toml && !print {
        write_cargo_toml(output)?;
    }

    info!("translation complete: {translated} translated, {skipped} skipped (unchanged)");

    if had_errors {
        anyhow::bail!("one or more files failed to translate");
    }

    Ok(())
}

// ── Legacy translate (backwards-compat with positional args) ────────────

fn translate_files_legacy(
    inputs: &[PathBuf],
    output: &Path,
    dump_ir: bool,
    print: bool,
) -> anyhow::Result<()> {
    for input in inputs {
        translate_one(input, output, dump_ir, print, false)?;
    }
    Ok(())
}

// ── Watch mode ──────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn run_watch_mode(
    input_paths: &[PathBuf],
    classpath: &[PathBuf],
    output: &Path,
    dump_ir: bool,
    print: bool,
    incremental: bool,
    gen_source_map: bool,
    cargo_toml: bool,
) -> anyhow::Result<()> {
    use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};
    use std::sync::mpsc;
    use std::time::Duration;

    info!("entering watch mode — press Ctrl+C to stop");

    let (tx, rx) = mpsc::channel();
    let mut debouncer =
        new_debouncer(Duration::from_millis(500), tx).context("creating file watcher")?;

    for path in input_paths {
        let canonical = std::fs::canonicalize(path)
            .with_context(|| format!("canonicalising watch path {path:?}"))?;
        debouncer
            .watcher()
            .watch(&canonical, notify::RecursiveMode::Recursive)
            .with_context(|| format!("watching {canonical:?}"))?;
        info!("watching {canonical:?}");
    }

    loop {
        match rx.recv() {
            Ok(Ok(events)) => {
                let java_events: Vec<_> = events
                    .iter()
                    .filter(|e| {
                        e.kind == DebouncedEventKind::Any
                            && (e.path.extension().and_then(|ext| ext.to_str()) == Some("java"))
                    })
                    .collect();

                if java_events.is_empty() {
                    continue;
                }

                info!(
                    "{} Java file(s) changed, re-translating…",
                    java_events.len()
                );

                // Re-collect all java files (input + classpath) and retranslate.
                let mut java_files = collect_java_files(input_paths)?;
                java_files.extend(collect_java_files(classpath).unwrap_or_default());
                if let Err(e) = translate_files(
                    &java_files,
                    output,
                    dump_ir,
                    print,
                    incremental,
                    gen_source_map,
                    cargo_toml,
                ) {
                    error!("translation error: {e:#}");
                }
            }
            Ok(Err(e)) => {
                error!("watch error: {e}");
            }
            Err(e) => {
                error!("watch channel error: {e}");
                break;
            }
        }
    }

    Ok(())
}

// ── Cargo.toml generation ───────────────────────────────────────────────

fn write_cargo_toml(output_dir: &Path) -> anyhow::Result<()> {
    let cargo_toml_path = output_dir.join("Cargo.toml");
    let content = r#"[package]
name = "translated-java"
version = "0.1.0"
edition = "2021"
description = "Rust code auto-generated by jtrans from Java sources"

[dependencies]
# The generated Rust code may rely on a `java-compat` support crate that
# provides Java standard-library shims. Add an appropriate dependency here
# that matches your setup, for example:
#
#   java-compat = { path = "../../java-compat" }
#   # or, if available on crates.io:
#   # java-compat = "0.1"
#
java-compat = { path = "java-compat" }
"#;
    std::fs::write(&cargo_toml_path, content)
        .with_context(|| format!("writing {cargo_toml_path:?}"))?;
    info!("wrote {cargo_toml_path:?}");
    Ok(())
}

// ── Maven plugin fragment ───────────────────────────────────────────────

fn write_maven_fragment(output_dir: &Path) -> anyhow::Result<()> {
    let pom_path = output_dir.join("jtrans-maven-plugin.xml");
    let content = r#"<!--
  jtrans Maven integration — add this <plugin> block to your <build><plugins> section.
  Requires `jtrans` on PATH (install via `cargo install jtrans`).

  Usage: mvn compile exec:exec@jtrans
-->
<plugin>
    <groupId>org.codehaus.mojo</groupId>
    <artifactId>exec-maven-plugin</artifactId>
    <version>3.1.0</version>
    <executions>
        <execution>
            <id>jtrans</id>
            <phase>generate-sources</phase>
            <goals>
                <goal>exec</goal>
            </goals>
            <configuration>
                <executable>jtrans</executable>
                <arguments>
                    <argument>translate</argument>
                    <argument>--input</argument>
                    <argument>${project.basedir}/src/main/java</argument>
                    <argument>--output</argument>
                    <argument>${project.build.directory}/rust-out</argument>
                    <argument>--classpath</argument>
                    <argument>${project.build.outputDirectory}</argument>
                </arguments>
            </configuration>
        </execution>
    </executions>
</plugin>
"#;
    std::fs::create_dir_all(output_dir)?;
    std::fs::write(&pom_path, content)
        .with_context(|| format!("writing Maven fragment {pom_path:?}"))?;
    info!("wrote Maven plugin fragment to {pom_path:?}");
    println!("Maven plugin fragment written to {pom_path:?}");
    println!("Copy the contents into your pom.xml <build><plugins> section.");
    Ok(())
}

// ── Gradle plugin fragment ──────────────────────────────────────────────

fn write_gradle_fragment(output_dir: &Path) -> anyhow::Result<()> {
    let gradle_path = output_dir.join("jtrans.gradle.kts");
    let content = r#"// jtrans Gradle integration — apply this script in your build.gradle.kts:
//   apply(from = "jtrans.gradle.kts")
//
// Requires `jtrans` on PATH (install via `cargo install jtrans`).
// Usage: ./gradlew translateToRust

tasks.register<Exec>("translateToRust") {
    group = "build"
    description = "Translate Java sources to Rust using jtrans"

    val inputDir = file("src/main/java")
    val outputDir = layout.buildDirectory.dir("rust-out").get().asFile

    inputs.dir(inputDir)
    outputs.dir(outputDir)

    commandLine(
        "jtrans",
        "translate",
        "--input", inputDir.absolutePath,
        "--output", outputDir.absolutePath,
        "--classpath", layout.buildDirectory.dir("classes/java/main").get().asFile.absolutePath
    )

    dependsOn("compileJava")
}
"#;
    std::fs::create_dir_all(output_dir)?;
    std::fs::write(&gradle_path, content)
        .with_context(|| format!("writing Gradle fragment {gradle_path:?}"))?;
    info!("wrote Gradle plugin fragment to {gradle_path:?}");
    println!("Gradle plugin fragment written to {gradle_path:?}");
    println!("Add `apply(from = \"jtrans.gradle.kts\")` to your build.gradle.kts.");
    Ok(())
}

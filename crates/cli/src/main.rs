//! `jtrans` — command-line driver for the Java→Rust translator.

use anyhow::Context;
use clap::Parser;
use std::path::PathBuf;
use tracing::info;

#[derive(Parser, Debug)]
#[command(
    name = "jtrans",
    version,
    about = "Translate Java source files to idiomatic, memory-safe Rust"
)]
struct Cli {
    /// Java source file(s) to translate.
    #[arg(required = true)]
    inputs: Vec<PathBuf>,

    /// Output directory for the generated Rust crate.
    #[arg(short, long, default_value = "out")]
    output: PathBuf,

    /// Print the IR as JSON after parsing (debugging aid).
    #[arg(long)]
    dump_ir: bool,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("jtrans=info".parse().unwrap()),
        )
        .init();

    let cli = Cli::parse();

    std::fs::create_dir_all(&cli.output)
        .with_context(|| format!("creating output directory {:?}", cli.output))?;

    for input in &cli.inputs {
        let source =
            std::fs::read_to_string(input).with_context(|| format!("reading {input:?}"))?;

        info!("parsing {input:?}");
        let mut module =
            parser::parse_source(&source).with_context(|| format!("parsing {input:?}"))?;

        if cli.dump_ir {
            println!("{}", serde_json::to_string_pretty(&module)?);
        }

        info!("type-checking {input:?}");
        let errors = typeck::check(&mut module);
        if !errors.is_empty() {
            for e in &errors {
                eprintln!("typeck warning: {e}");
            }
        }

        info!("generating Rust code for {input:?}");
        let tokens =
            codegen::generate(&module).with_context(|| format!("codegen for {input:?}"))?;

        // Determine output file name: strip .java, use class name as file name
        let stem = input
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        let out_rs = cli.output.join(format!("{stem}.rs"));

        // Format with rustfmt if available; otherwise write raw tokens
        let rust_src = prettyprint(&tokens.to_string());
        std::fs::write(&out_rs, &rust_src)
            .with_context(|| format!("writing {out_rs:?}"))?;

        info!("wrote {out_rs:?}");
    }

    Ok(())
}

/// Attempt to format `src` with rustfmt. Falls back to the raw string on failure.
fn prettyprint(src: &str) -> String {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut child = match Command::new("rustfmt")
        .args(["--edition", "2021"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return src.to_owned(),
    };

    if let Some(stdin) = child.stdin.as_mut() {
        let _ = stdin.write_all(src.as_bytes());
    }
    let output = match child.wait_with_output() {
        Ok(o) => o,
        Err(_) => return src.to_owned(),
    };
    if output.status.success() {
        String::from_utf8(output.stdout).unwrap_or_else(|_| src.to_owned())
    } else {
        src.to_owned()
    }
}


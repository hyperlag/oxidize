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

    for input in &cli.inputs {
        let source =
            std::fs::read_to_string(input).with_context(|| format!("reading {input:?}"))?;

        info!("parsing {input:?}");
        let _tree = parser::parse_source(&source).with_context(|| format!("parsing {input:?}"))?;

        info!("parse OK — full lowering will be implemented in Stage 1");
    }

    Ok(())
}

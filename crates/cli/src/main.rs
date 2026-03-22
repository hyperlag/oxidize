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
        let ir_module =
            parser::parse_to_ir(&source).with_context(|| format!("parsing {input:?}"))?;

        if cli.dump_ir {
            let json = serde_json::to_string_pretty(&ir_module)
                .context("serialising IR to JSON")?;
            println!("{json}");
        }

        info!("type-checking {input:?}");
        let typed_module = typeck::type_check(ir_module)
            .with_context(|| format!("type-checking {input:?}"))?;

        info!("generating Rust for {input:?}");
        let rust_src = codegen::generate(&typed_module)
            .with_context(|| format!("code-generating {input:?}"))?;

        if cli.print {
            println!("{rust_src}");
        } else {
            std::fs::create_dir_all(&cli.output)
                .with_context(|| format!("creating output dir {:?}", cli.output))?;

            let stem = input
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("output");
            let out_path = cli.output.join(format!("{}.rs", stem.to_lowercase()));
            std::fs::write(&out_path, &rust_src)
                .with_context(|| format!("writing {out_path:?}"))?;
            info!("wrote {out_path:?}");
        }
    }

    Ok(())
}

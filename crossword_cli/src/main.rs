use anyhow::Result;
use clap::Parser;
use crossword_cli::{run_cli, CliArgs};

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let args = CliArgs::parse();
    run_cli(args)
}

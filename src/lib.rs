#![forbid(unsafe_code)]

use clap::Parser;

pub mod app;
pub mod domain;
pub mod infra;
pub mod tui;

pub fn run() -> anyhow::Result<()> {
    let cli = app::cli::Cli::parse();
    app::commands::run(cli)
}

#![allow(clippy::print_stdout)]

use agentdriver::harness::recipes;
use clap::{Parser, Subcommand};

#[cfg(feature = "mcp")]
mod mcp;

#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// List the harnesses this build knows.
    Harnesses,
    /// Serve the verbs over MCP on stdio.
    #[cfg(feature = "mcp")]
    Mcp,
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Harnesses => {
            for recipe in recipes() {
                println!("{}", recipe.kind);
            }
            Ok(())
        }
        // Only this subcommand starts a runtime; the others never pay for it.
        #[cfg(feature = "mcp")]
        Command::Mcp => tokio::runtime::Runtime::new()?.block_on(mcp::serve()),
    }
}

//! CLI definition and entrypoint to executable

use crate::{
    db, node, stage, test_eth_chain,
    util::reth_tracing::{self, TracingMode},
};
use clap::{ArgAction, Parser, Subcommand};
use tracing_subscriber::util::SubscriberInitExt;

/// main function that parses cli and runs command
pub async fn run() -> eyre::Result<()> {
    let opt = Cli::parse();

    let tracing = if opt.silent { TracingMode::Silent } else { TracingMode::All };

    reth_tracing::build_subscriber(tracing).init();

    match opt.command {
        Commands::Node(command) => command.execute().await,
        Commands::TestEthChain(command) => command.execute().await,
        Commands::Db(command) => command.execute().await,
        Commands::Stage(command) => command.execute().await,
    }
}

/// Commands to be executed
#[derive(Subcommand)]
pub enum Commands {
    /// Start the node
    #[command(name = "node")]
    Node(node::Command),
    /// Run Ethereum blockchain tests
    #[command(name = "test-chain")]
    TestEthChain(test_eth_chain::Command),
    /// Database debugging utilities
    #[command(name = "db")]
    Db(db::Command),
    /// Run a single stage
    #[command(name = "stage")]
    Stage(stage::Command),
}

#[derive(Parser)]
#[command(author, version="0.1", about="Reth binary", long_about = None)]
struct Cli {
    /// The command to run
    #[clap(subcommand)]
    command: Commands,

    /// Use verbose output
    #[clap(short, long, action = ArgAction::Count, global = true)]
    verbose: u8,

    /// Silence all output
    #[clap(long, global = true)]
    silent: bool,
}

use clap::Parser;
use ethers::prelude::Address;
use stderrlog::Timestamp;

use crate::server::run;

mod server;

#[derive(Clone, Debug, Parser)]
#[command(name = "adm_faucet", author, version, about, long_about = None)]
struct Cli {
    /// Wallet private key (ECDSA, secp256k1) for sending faucet funds.
    #[arg(short, long, env)]
    private_key: String,
    /// Faucet token address.
    #[arg(long, env)]
    token_address: Address,
    /// Logging verbosity (repeat for more verbose logging).
    #[arg(short, long, env, action = clap::ArgAction::Count)]
    verbosity: u8,
    /// Silence logging.
    #[arg(short, long, env, default_value_t = false)]
    quiet: bool,
    /// EVM RPC URL.
    #[arg(long, env, default_value_t = String::from("http://localhost:8545"))]
    rpc_url: String,
    /// Listen port for the faucet service.
    #[arg(long, env, default_value_t = 8080)]
    port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    stderrlog::new()
        .module(module_path!())
        .quiet(cli.quiet)
        .verbosity(cli.verbosity as usize)
        .timestamp(Timestamp::Millisecond)
        .init()
        .unwrap();

    run(cli).await
}

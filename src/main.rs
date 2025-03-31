use std::net::{IpAddr, SocketAddr};

use clap::{Parser, Subcommand};
use ethers::prelude::Address;
use stderrlog::Timestamp;

use crate::server::run;

mod server;

#[derive(Clone, Debug, Parser)]
#[command(name = "registrar", author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    /// Wallet private key (ECDSA, secp256k1) used to register new accounts and send drips.
    #[arg(short, long, env)]
    private_key: String,
    /// Cloudflare secret key.
    #[arg(short, long, env)]
    ts_secret_key: String,
    /// IP address of the proxy server this is running behind.
    #[arg(long, env, value_delimiter = ',')]
    trusted_proxy_ips: Vec<IpAddr>,
    /// RECALL faucet contract address.
    #[arg(long, env)]
    faucet_address: Address,
    /// Target chain Ethereum RPC URL.
    #[arg(long, env, default_value = "http://127.0.0.1:8545")]
    evm_rpc_url: String,
    /// Host the service will bind to.
    #[arg(long, env, default_value = "127.0.0.1")]
    listen_host: String,
    /// Port the service will bind to.
    #[arg(long, env, default_value_t = 8080)]
    listen_port: u16,
    /// Logging verbosity (repeat for more verbose logging).
    #[arg(short, long, env, action = clap::ArgAction::Count, default_value = "2")]
    verbosity: u8,
    /// Silence logging.
    #[arg(short, long, env, default_value_t = false)]
    quiet: bool,

    /// Prometheus metrics socket address
    #[arg(long, env, default_value = "127.0.0.1:9090")]
    metrics_listen_address: Option<SocketAddr>,
}

#[derive(Clone, Debug, Subcommand)]
enum Commands {
    /// Start the registration service.
    Start,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    stderrlog::new()
        .module(module_path!())
        .quiet(cli.quiet)
        .verbosity(cli.verbosity as usize)
        .timestamp(Timestamp::Millisecond)
        .init()?;

    run(cli).await
}

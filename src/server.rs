use std::sync::Arc;

use anyhow::Context;
use cf_turnstile::TurnstileClient;
use ethers::prelude::{
    Http, LocalWallet, Middleware, NonceManagerMiddleware, Provider, Signer, SignerMiddleware,
};
use log::info;
use util::log_failed_request;
use warp::{Filter, Rejection, Reply};

use crate::server::shared::{DefaultSignerMiddleware, Faucet, FaucetContract};
use crate::Cli;

mod drip;
mod register;
mod shared;
mod util;

/// Server entrypoint for the service.
pub async fn run(cli: Cli) -> anyhow::Result<()> {
    let private_key = cli
        .private_key
        .strip_prefix("0x")
        .unwrap_or(&cli.private_key);
    let private_key = hex::decode(private_key)?;
    let trusted_proxy_ips = cli.trusted_proxy_ips;
    let faucet_address = cli.faucet_address;
    let evm_rpc_url = cli.evm_rpc_url;

    let provider = Provider::<Http>::try_from(evm_rpc_url)?;
    let chain_id = provider.get_chainid().await?.as_u64();
    let wallet = LocalWallet::from_bytes(&private_key)?.with_chain_id(chain_id);

    let nonce_manager = Arc::new(NonceManagerMiddleware::new(provider, wallet.address()));
    let client: DefaultSignerMiddleware = SignerMiddleware::new(nonce_manager, wallet);
    let client = Arc::new(client);

    let faucet: Faucet = FaucetContract::new(faucet_address, client.clone());
    let turnstile = TurnstileClient::new(cli.ts_secret_key.into());

    let health_route = warp::path!("health")
        .and(warp::get())
        .and_then(handle_health);
    let register_route = register::register_route(client.clone());
    let drip_route = drip::drip_route(trusted_proxy_ips, faucet, Arc::new(turnstile));
    let log = warp::log::custom(log_failed_request);
    let request_metrics = warp::log::custom(util::request_metrics);

    if let Some(metrics_addr) = cli.metrics_listen_address {
        let builder = prometheus_exporter::Builder::new(metrics_addr);
        let _ = builder.start().context("failed to start metrics server")?;
        info!("running metrics endpoint on {metrics_addr}");
    }

    let router = health_route
        .or(register_route)
        .or(drip_route)
        .recover(shared::handle_rejection)
        .with(
            warp::cors()
                .allow_any_origin()
                .allow_headers(vec!["Content-Type"])
                .allow_methods(vec!["GET", "POST"]),
        )
        .with(request_metrics)
        .with(log);

    let listen_addr = format!("{}:{}", cli.listen_host, cli.listen_port);
    info!("service listening on {}", listen_addr);
    let socket_addr: std::net::SocketAddr = listen_addr.parse()?;
    warp::serve(router).run(socket_addr).await;
    Ok(())
}

async fn handle_health() -> Result<impl Reply, Rejection> {
    Ok(warp::reply::reply())
}

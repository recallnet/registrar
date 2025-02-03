use std::sync::Arc;

use cf_turnstile::TurnstileClient;
use ethers::prelude::{Http, LocalWallet, Middleware, Provider, Signer, SignerMiddleware};
use log::info;
use shared::GoogleSheetsConfig;
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
    let client: DefaultSignerMiddleware = SignerMiddleware::new(provider, wallet);
    let client = Arc::new(client);
    let faucet: Faucet = FaucetContract::new(faucet_address, client.clone());
    let turnstile = TurnstileClient::new(cli.ts_secret_key.into());
    let google_sheets_config = GoogleSheetsConfig {
        google_sheets_api_key: cli.google_sheets_api_key,
        allowlist_spreadsheet_id: cli.allowlist_spreadsheet_id,
    };

    let health_route = warp::path!("health")
        .and(warp::get())
        .and_then(handle_health);
    let register_route = register::register_route(client.clone());
    let drip_route = drip::drip_route(
        trusted_proxy_ips,
        faucet,
        Arc::new(turnstile),
        Arc::new(google_sheets_config),
    );
    let log = warp::log::custom(log_failed_request);

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

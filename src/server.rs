use std::sync::Arc;

use ethers::prelude::{Http, LocalWallet, Middleware, Provider, Signer, SignerMiddleware};
use log::info;
use util::log_request_details;
use warp::Filter;

use crate::server::shared::{DefaultSignerMiddleware, Faucet, FaucetContract};
use crate::Cli;

mod register;
mod shared;
mod util;

/// Server entrypoint for the service.
pub async fn run(cli: Cli) -> anyhow::Result<()> {
    let private_key = hex::decode(cli.private_key)?;
    let faucet_address = cli.faucet_address;
    let evm_rpc_url = cli.evm_rpc_url;

    let provider = Provider::<Http>::try_from(evm_rpc_url)?;
    let chain_id = provider.get_chainid().await?.as_u64();
    let wallet = LocalWallet::from_bytes(&private_key)?.with_chain_id(chain_id);
    let client: DefaultSignerMiddleware = SignerMiddleware::new(provider, wallet);
    let client = Arc::new(client);
    let faucet: Faucet = FaucetContract::new(faucet_address, client);

    let register_route = register::register_route(faucet);
    let log_request_details = warp::log::custom(log_request_details);

    let router = register_route
        .with(
            warp::cors()
                .allow_any_origin()
                .allow_headers(vec!["Content-Type"])
                .allow_methods(vec!["POST"]),
        )
        .with(log_request_details)
        .recover(shared::handle_rejection);

    let listen_addr = format!("{}:{}", cli.listen_host, cli.listen_port);
    info!("service listening on {}", listen_addr);
    let socket_addr: std::net::SocketAddr = listen_addr.parse()?;
    warp::serve(router).run(socket_addr).await;
    Ok(())
}

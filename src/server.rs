use log::info;
use util::log_request_details;
use warp::Filter;

use crate::Cli;

mod send;
mod shared;
mod util;

/// Server entrypoint for the faucet service.
pub async fn run(cli: Cli) -> anyhow::Result<()> {
    let faucet_pk = cli.private_key;
    let token_address = cli.token_address;
    let send_route = send::send_route(faucet_pk.clone(), token_address);
    let log_request_details = warp::log::custom(log_request_details);
    let listen_addr = "0.0.0.0:8080";

    let router = send_route
        .with(
            warp::cors()
                .allow_any_origin()
                .allow_headers(vec!["Content-Type"])
                .allow_methods(vec!["POST"]),
        )
        .with(log_request_details)
        .recover(shared::handle_rejection);

    info!("Starting server at {}", listen_addr);
    let socket_addr: std::net::SocketAddr = listen_addr.parse()?;
    warp::serve(router).run(socket_addr).await;
    Ok(())
}

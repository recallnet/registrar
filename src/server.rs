use log::info;
use util::log_request_details;
use warp::Filter;

use crate::Cli;

mod register;
mod shared;
mod util;

/// Server entrypoint for the service.
pub async fn run(cli: Cli) -> anyhow::Result<()> {
    let service_sk = cli.private_key;
    let evm_rpc_url = cli.evm_rpc_url;
    let send_route = register::register_route(service_sk, evm_rpc_url);
    let log_request_details = warp::log::custom(log_request_details);

    let router = send_route
        .with(
            warp::cors()
                .allow_any_origin()
                .allow_headers(vec!["Content-Type"])
                .allow_methods(vec!["POST"]),
        )
        .with(log_request_details)
        .recover(shared::handle_rejection);

    let listen_addr = format!("{}:{}", cli.listen_host, cli.listen_port);
    info!("Service listening on {}", listen_addr);
    let socket_addr: std::net::SocketAddr = listen_addr.parse()?;
    warp::serve(router).run(socket_addr).await;
    Ok(())
}

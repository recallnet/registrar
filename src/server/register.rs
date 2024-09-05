use anyhow::anyhow;
use ethers::prelude::{Address, TransactionReceipt};
use serde_json::json;
use warp::{Filter, Rejection, Reply};

use crate::server::shared::Faucet;
use crate::server::{
    shared::{with_faucet, BadRequest, BaseRequest},
    util::log_request_body,
};

/// Route filter for `/register` endpoint.
pub fn register_route(
    faucet: Faucet,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path("register")
        .and(warp::post())
        .and(warp::header::exact("content-type", "application/json"))
        .and(warp::body::json())
        .and(with_faucet(faucet))
        .and_then(handle_register)
}

/// Handles the `/register` request.
pub async fn handle_register(
    req: BaseRequest,
    faucet: Faucet,
) -> anyhow::Result<impl Reply, Rejection> {
    log_request_body("register", &format!("{}", req));

    let to_address = req.address.parse::<Address>().map_err(|e| {
        Rejection::from(BadRequest {
            message: format!("invalid ethereum address: {}", e),
        })
    })?;

    let res = drip(faucet, to_address).await.map_err(|e| {
        Rejection::from(BadRequest {
            message: format!("register error: {}", e),
        })
    })?;
    let json = json!({"tx_hash": res.transaction_hash});
    Ok(warp::reply::json(&json))
}

/// Drips a small amount of HOKU to an address on the subnet using the faucet.
/// This will trigger the FVM to create an account for the address.
pub async fn drip(faucet: Faucet, to_address: Address) -> anyhow::Result<TransactionReceipt> {
    let tx = faucet.drip(to_address);
    let tx_pending = tx.send().await?;
    tx_pending
        .await?
        .ok_or(anyhow!("drip did not return a receipt"))
}

use crate::server::shared::Provider;
use crate::server::{
    shared::{with_client, BadRequest, RegisterRequest},
    util::log_request_body,
};
use alloy::{
    primitives::{Address, TxHash, U256},
    providers::Provider as AlloyProvider,
    rpc::types::eth::TransactionRequest,
};
use anyhow::anyhow;
use serde_json::json;
use std::sync::Arc;
use warp::{Filter, Rejection, Reply};

/// Enum to handle register results.
enum RegisterResult {
    Pending(TxHash),
    Success(TxHash),
    Failure(String),
}

/// Route filter for `/register` endpoint.
pub fn register_route(
    client: Arc<Provider>,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path("register")
        .and(warp::post())
        .and(warp::header::exact("content-type", "application/json"))
        .and(warp::body::json())
        .and(with_client(client))
        .and_then(handle_register)
}

/// Handles the `/register` request.
pub async fn handle_register(
    req: RegisterRequest,
    client: Arc<Provider>,
) -> anyhow::Result<impl Reply, Rejection> {
    log_request_body("register", &format!("{}", req));

    let to_address = req.address.parse::<Address>().map_err(|e| {
        Rejection::from(BadRequest {
            message: format!("invalid ethereum address: {}", e),
        })
    })?;

    let res = register(client, to_address, req.wait).await.map_err(|e| {
        Rejection::from(BadRequest {
            message: format!("register error: {}", e),
        })
    })?;
    match res {
        RegisterResult::Success(tx) | RegisterResult::Pending(tx) => {
            Ok(warp::reply::json(&json!({"tx_hash": tx})))
        }
        RegisterResult::Failure(message) => Err(warp::reject::custom(BadRequest { message })),
    }
}

/// Registers an address on the subnet by sending a transaction.
/// This will trigger the FVM to create an account for the address.
async fn register(
    client: Arc<Provider>,
    to_address: Address,
    wait: Option<bool>,
) -> anyhow::Result<RegisterResult> {
    let tx = TransactionRequest::default()
        .to(to_address)
        .value(U256::ZERO);
    let tx_pending = client.send_transaction(tx).await;

    match tx_pending {
        Ok(tx) => {
            let hash = tx.tx_hash().clone();
            let wait = wait.unwrap_or(true);
            if wait {
                tx.get_receipt()
                    .await
                    .map_err(|e| anyhow!("error getting receipt: {}", e))?;
                Ok(RegisterResult::Success(hash))
            } else {
                Ok(RegisterResult::Pending(hash))
            }
        }
        Err(e) => Ok(RegisterResult::Failure(e.to_string())),
    }
}

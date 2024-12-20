use crate::server::shared::DefaultSignerMiddleware;
use crate::server::{
    shared::{with_client, BadRequest, RegisterRequest},
    util::log_request_body,
};
use anyhow::anyhow;
use ethers::{
    core::types::TransactionRequest,
    prelude::{Address, TxHash},
    providers::Middleware,
};
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
    client: Arc<DefaultSignerMiddleware>,
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
    client: Arc<DefaultSignerMiddleware>,
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
    client: Arc<DefaultSignerMiddleware>,
    to_address: Address,
    wait: Option<bool>,
) -> anyhow::Result<RegisterResult> {
    let tx = TransactionRequest::new().to(to_address);
    let tx_pending = client.send_transaction(tx, None).await;
    match tx_pending {
        Ok(tx) => {
            let hash = tx.tx_hash();
            let wait = wait.unwrap_or(true);
            if wait {
                tx.await?
                    .ok_or(anyhow!("register did not return a receipt"))?;
                Ok(RegisterResult::Success(hash))
            } else {
                Ok(RegisterResult::Pending(hash))
            }
        }
        Err(e) => Ok(RegisterResult::Failure(e.to_string())),
    }
}

use crate::server::shared::{DefaultSignerMiddleware, Faucet, FaucetEmpty, TooManyRequests};
use crate::server::{
    shared::{with_faucet, BadRequest, RegisterRequest},
    util::log_request_body,
};
use anyhow::anyhow;
use ethers::prelude::{Address, ContractError, TxHash};
use ethers::utils::keccak256;
use once_cell::sync::Lazy;
use serde_json::json;
use warp::{Filter, Rejection, Reply};

static TRY_LATER_SELECTOR: Lazy<Vec<u8>> = Lazy::new(|| keccak256(b"TryLater()")[0..4].into());
static FAUCET_EMPTY_SELECTOR: Lazy<Vec<u8>> =
    Lazy::new(|| keccak256(b"FaucetEmpty()")[0..4].into());

/// Enum to handle drip results.
enum DripResult {
    Pending(TxHash),
    Success(TxHash),
    Failure(String),
    RateLimited,
    FaucetEmpty,
}

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
    req: RegisterRequest,
    faucet: Faucet,
) -> anyhow::Result<impl Reply, Rejection> {
    log_request_body("register", &format!("{}", req));

    let to_address = req.address.parse::<Address>().map_err(|e| {
        Rejection::from(BadRequest {
            message: format!("invalid ethereum address: {}", e),
        })
    })?;

    let res = drip(faucet, to_address, req.wait).await.map_err(|e| {
        Rejection::from(BadRequest {
            message: format!("register error: {}", e),
        })
    })?;
    match res {
        DripResult::Success(tx) | DripResult::Pending(tx) => {
            Ok(warp::reply::json(&json!({"tx_hash": tx})))
        }
        DripResult::Failure(message) => Err(warp::reject::custom(BadRequest { message })),
        DripResult::RateLimited => Err(warp::reject::custom(TooManyRequests {})),
        DripResult::FaucetEmpty => Err(warp::reject::custom(FaucetEmpty {})),
    }
}

/// Drips a small amount of HOKU to an address on the subnet using the faucet.
/// This will trigger the FVM to create an account for the address.
async fn drip(
    faucet: Faucet,
    to_address: Address,
    wait: Option<bool>,
) -> anyhow::Result<DripResult> {
    let tx = faucet.drip(to_address);
    let tx_pending = tx.send().await;
    match tx_pending {
        Ok(tx) => {
            let hash = tx.tx_hash();
            let wait = if let Some(wait) = wait { wait } else { true };
            if wait {
                tx.await?.ok_or(anyhow!("drip did not return a receipt"))?;
                Ok(DripResult::Success(hash))
            } else {
                Ok(DripResult::Pending(hash))
            }
        }
        Err(e) => Ok(result_from_error(e)),
    }
}

fn result_from_error(err: ContractError<DefaultSignerMiddleware>) -> DripResult {
    if let Some(data) = err.as_revert() {
        if data.len() < 4 {
            return DripResult::Failure(err.to_string());
        }
        let selector = &data[..4];
        if selector == *TRY_LATER_SELECTOR {
            DripResult::RateLimited
        } else if selector == *FAUCET_EMPTY_SELECTOR {
            DripResult::FaucetEmpty
        } else {
            DripResult::Failure(err.to_string())
        }
    } else {
        DripResult::Failure(err.to_string())
    }
}

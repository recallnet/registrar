use crate::server::shared::DefaultSignerMiddleware;
use crate::server::{
    shared::{with_client, BadRequest, RegisterRequest},
    util::log_request_body,
};
use anyhow::anyhow;
use ethers::{
    core::types::Eip1559TransactionRequest,
    prelude::{Address, TxHash, I256, U256},
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
    let (fee, fee_cap) = premium_estimation(client.clone()).await?;
    let tx = Eip1559TransactionRequest::new()
        .to(to_address)
        .value(U256::zero())
        .max_priority_fee_per_gas(fee)
        .max_fee_per_gas(fee_cap);
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

/// Returns an estimation of an optimal `gas_premium` and `gas_fee_cap`
/// for a transaction considering the average premium, base_fee and reward percentile from
/// past blocks
/// This is an adaptation of ethers' `eip1559_default_estimator`:
/// https://github.com/gakonst/ethers-rs/blob/5dcd3b7e754174448f9a8cbfc0523896609629f9/ethers-core/src/utils/mod.rs#L476
async fn premium_estimation(signer: Arc<DefaultSignerMiddleware>) -> anyhow::Result<(U256, U256)> {
    let base_fee_per_gas = signer
        .get_block(ethers::types::BlockNumber::Latest)
        .await?
        .ok_or_else(|| anyhow!("Latest block not found"))?
        .base_fee_per_gas
        .ok_or_else(|| anyhow!("EIP-1559 not activated"))?;

    let fee_history = signer
        .fee_history(
            ethers::utils::EIP1559_FEE_ESTIMATION_PAST_BLOCKS,
            ethers::types::BlockNumber::Latest,
            &[ethers::utils::EIP1559_FEE_ESTIMATION_REWARD_PERCENTILE],
        )
        .await?;

    let max_priority_fee_per_gas = estimate_priority_fee(fee_history.reward); //overestimate?
    let potential_max_fee = base_fee_surged(base_fee_per_gas);
    let max_fee_per_gas = if max_priority_fee_per_gas > potential_max_fee {
        max_priority_fee_per_gas + potential_max_fee
    } else {
        potential_max_fee
    };

    Ok((max_priority_fee_per_gas, max_fee_per_gas))
}

/// Implementation borrowed from
/// https://github.com/gakonst/ethers-rs/blob/ethers-v2.0.8/ethers-core/src/utils/mod.rs#L582
/// Refer to the implementation for unit tests
fn base_fee_surged(base_fee_per_gas: U256) -> U256 {
    if base_fee_per_gas <= U256::from(40_000_000_000u64) {
        base_fee_per_gas * 2
    } else if base_fee_per_gas <= U256::from(100_000_000_000u64) {
        base_fee_per_gas * 16 / 10
    } else if base_fee_per_gas <= U256::from(200_000_000_000u64) {
        base_fee_per_gas * 14 / 10
    } else {
        base_fee_per_gas * 12 / 10
    }
}

/// Implementation borrowed from
/// https://github.com/gakonst/ethers-rs/blob/ethers-v2.0.8/ethers-core/src/utils/mod.rs#L536
/// Refer to the implementation for unit tests
fn estimate_priority_fee(rewards: Vec<Vec<U256>>) -> U256 {
    let mut rewards: Vec<U256> = rewards
        .iter()
        .map(|r| r[0])
        .filter(|r| *r > U256::zero())
        .collect();
    if rewards.is_empty() {
        return U256::zero();
    }
    if rewards.len() == 1 {
        return rewards[0];
    }
    // Sort the rewards as we will eventually take the median.
    rewards.sort();

    // A copy of the same vector is created for convenience to calculate percentage change
    // between subsequent fee values.
    let mut rewards_copy = rewards.clone();
    rewards_copy.rotate_left(1);

    let mut percentage_change: Vec<I256> = rewards
        .iter()
        .zip(rewards_copy.iter())
        .map(|(a, b)| {
            let a = I256::try_from(*a).expect("priority fee overflow");
            let b = I256::try_from(*b).expect("priority fee overflow");
            ((b - a) * 100) / a
        })
        .collect();
    percentage_change.pop();

    // Fetch the max of the percentage change, and that element's index.
    let max_change = percentage_change.iter().max().unwrap();
    let max_change_index = percentage_change
        .iter()
        .position(|&c| c == *max_change)
        .unwrap();

    // If we encountered a big change in fees at a certain position, then consider only
    // the values >= it.
    let values = if *max_change >= ethers::utils::EIP1559_FEE_ESTIMATION_THRESHOLD_MAX_CHANGE.into()
        && (max_change_index >= (rewards.len() / 2))
    {
        rewards[max_change_index..].to_vec()
    } else {
        rewards
    };

    // Return the median.
    values[values.len() / 2]
}

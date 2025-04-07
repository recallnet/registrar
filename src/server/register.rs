use crate::server::shared::Provider;
use crate::server::{
    shared::{with_client, BadRequest, RegisterRequest},
    util::log_request_body,
};
use alloy::eips::{BlockId, BlockNumberOrTag};
use alloy::{
    primitives::{Address, U256, TxHash},
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
    let (fee, fee_cap) = premium_estimation(client.clone()).await?;
    let tx = TransactionRequest::default()
        .to(to_address)
        .value(U256::ZERO)
        .max_priority_fee_per_gas(fee)
        .max_fee_per_gas(fee_cap);
    let tx_pending = client.send_transaction(tx).await;

    match tx_pending {
        Ok(tx) => {
            let hash = tx.tx_hash().clone();
            let wait = wait.unwrap_or(true);
            if wait {
                tx.get_receipt().await.map_err(|e| anyhow!("error getting receipt: {}", e))?;
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
async fn premium_estimation(provider: Arc<Provider>) -> anyhow::Result<(u128, u128)> {
    let block = provider
        .get_block(BlockId::latest())
        .await?
        .ok_or_else(|| anyhow!("Latest block not found"))?;
    
    let base_fee_per_gas = block
        .header
        .base_fee_per_gas
        .ok_or_else(|| anyhow!("EIP-1559 not activated"))?;

    let fee_history = provider.get_fee_history(
        10, 
        BlockNumberOrTag::Latest, 
        &[5.0])
    .await?;

    let max_priority_fee_per_gas = estimate_priority_fee(fee_history.reward.unwrap()); //overestimate?
    let potential_max_fee = base_fee_surged(u128::from(base_fee_per_gas));
    let max_fee_per_gas = if max_priority_fee_per_gas > potential_max_fee {
        max_priority_fee_per_gas + potential_max_fee
    } else {
        potential_max_fee
    };

    Ok((max_priority_fee_per_gas, max_fee_per_gas))
}

/// Implementation borrowed from ethers-rs
fn base_fee_surged(base_fee_per_gas: u128) -> u128 {
    if base_fee_per_gas <= 40_000_000_000u128 {
        base_fee_per_gas * 2u128
    } else if base_fee_per_gas <= 100_000_000_000u128 {
        base_fee_per_gas * 16u128 / 10u128
    } else if base_fee_per_gas <= 200_000_000_000u128 {
        base_fee_per_gas * 14u128 / 10u128
    } else {
        base_fee_per_gas * 12u128 / 10u128
    }
}

/// Implementation borrowed from
/// https://github.com/gakonst/ethers-rs/blob/ethers-v2.0.8/ethers-core/src/utils/mod.rs#L536
/// Refer to the implementation for unit tests
fn estimate_priority_fee(rewards: Vec<Vec<u128>>) -> u128 {
    let mut rewards: Vec<u128> = rewards
        .iter()
        .map(|r| r[0])
        .filter(|r| *r > 0)
        .collect();
    if rewards.is_empty() {
        return 0;
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

    let mut percentage_change: Vec<u128> = rewards
        .iter()
        .zip(rewards_copy.iter())
        .map(|(a, b)| {
            let a = u128::try_from(*a).expect("priority fee overflow");
            let b = u128::try_from(*b).expect("priority fee overflow");
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
    let values = if *max_change >= 200
        && (max_change_index >= (rewards.len() / 2))
    {
        rewards[max_change_index..].to_vec()
    } else {
        rewards
    };

    // Return the median.
    values[values.len() / 2]
}

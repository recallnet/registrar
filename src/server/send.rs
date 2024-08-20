use ethers::prelude::{
    abigen, Address, Http, LocalWallet, Middleware, Provider, Signer, SignerMiddleware, TxHash,
};
use serde_json::json;
use std::convert::TryFrom;
use std::error::Error;
use std::sync::Arc;
use warp::{Filter, Rejection, Reply};

use crate::server::{
    shared::{with_private_key, with_rpc_url, with_token_address, BadRequest, BaseRequest},
    util::log_request_body,
};

abigen!(
    tHoku,
    r#"[{"inputs":[{"internalType":"address","name":"to","type":"address"},{"internalType":"uint256","name":"amount","type":"uint256"}],"name":"mint","outputs":[],"stateMutability":"nonpayable","type":"function"}]"#
);

/// Amount to send from the faucet to the user.
const FAUCET_AMOUNT: u64 = 5_000_000_000_000_000_000;

/// Route filter for `/send` endpoint.
pub fn send_route(
    private_key: String,
    token_address: Address,
    rpc_url: String,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path("send")
        .and(warp::post())
        .and(warp::header::exact("content-type", "application/json"))
        .and(warp::body::json())
        .and(with_private_key(private_key.clone()))
        .and(with_token_address(token_address.clone()))
        .and(with_rpc_url(rpc_url.clone()))
        .and_then(handle_send)
}

/// Handles the `/send` request.
pub async fn handle_send(
    req: BaseRequest,
    private_key: String,
    token_address: Address,
    rpc_url: String,
) -> anyhow::Result<impl Reply, Rejection> {
    log_request_body("send", &format!("{}", req));

    let eth_address = req.address.parse::<Address>().map_err(|e| {
        Rejection::from(BadRequest {
            message: format!("Invalid Ethereum address: {}", e),
        })
    })?;

    let res = send(eth_address, private_key, token_address, rpc_url)
        .await
        .map_err(|e| {
            Rejection::from(BadRequest {
                message: format!("send error: {}", e),
            })
        })?;
    let json = json!(res);
    Ok(warp::reply::json(&json))
}

/// Sends a transaction on the subnet.
pub async fn send(
    address: Address,
    private_key: String,
    token_address: Address,
    rpc_url: String,
) -> anyhow::Result<TxHash, Box<dyn Error>> {
    let node_url = rpc_url;
    let provider = Provider::<Http>::try_from(node_url.to_string())?;
    let chain_id = provider.get_chainid().await?.as_u64();

    // Parse the private key from hex string
    let private_key_bytes = hex::decode(private_key)?;
    let wallet = LocalWallet::from_bytes(&private_key_bytes)?.with_chain_id(chain_id);

    // get balance of the given address
    let balance = provider.get_balance(address, None).await?;
    if balance.is_zero() {
        return Err("make sure the address has nonzero FIL balance".into());
    }

    let client = SignerMiddleware::new(provider, wallet);
    let contract = tHoku::new(token_address, Arc::new(client));
    let receipt = contract
        .mint(address, FAUCET_AMOUNT.into())
        .send()
        .await?
        .clone();

    Ok(receipt)
}

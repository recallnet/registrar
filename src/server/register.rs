use ethers::prelude::{
    Address, Http, LocalWallet, Middleware, Provider, Signer, SignerMiddleware, TransactionRequest,
    TxHash, U256,
};
use serde_json::json;
use std::convert::TryFrom;
use std::error::Error;
use warp::{Filter, Rejection, Reply};

use crate::server::{
    shared::{with_private_key, with_rpc_url, BadRequest, BaseRequest},
    util::log_request_body,
};

/// Route filter for `/register` endpoint.
pub fn register_route(
    private_key: String,
    rpc_url: String,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path("register")
        .and(warp::post())
        .and(warp::header::exact("content-type", "application/json"))
        .and(warp::body::json())
        .and(with_private_key(private_key.clone()))
        .and(with_rpc_url(rpc_url.clone()))
        .and_then(handle_register)
}

/// Handles the `/register` request.
pub async fn handle_register(
    req: BaseRequest,
    private_key: String,
    rpc_url: String,
) -> anyhow::Result<impl Reply, Rejection> {
    log_request_body("register", &format!("{}", req));

    let eth_address = req.address.parse::<Address>().map_err(|e| {
        Rejection::from(BadRequest {
            message: format!("Invalid Ethereum address: {}", e),
        })
    })?;

    let res = send_zero(eth_address, private_key, rpc_url)
        .await
        .map_err(|e| {
            Rejection::from(BadRequest {
                message: format!("register error: {}", e),
            })
        })?;
    let json = json!(res);
    Ok(warp::reply::json(&json))
}

/// Sends zero value to an address on the subnet.
/// This will trigger the FVM to create an account for the address.
pub async fn send_zero(
    address: Address,
    private_key: String,
    rpc_url: String,
) -> anyhow::Result<Option<TxHash>, Box<dyn Error>> {
    let node_url = rpc_url;
    let provider = Provider::<Http>::try_from(node_url.to_string())?;
    let chain_id = provider.get_chainid().await?.as_u64();

    // Parse the private key from hex string
    let private_key_bytes = hex::decode(private_key)?;
    let wallet = LocalWallet::from_bytes(&private_key_bytes)?.with_chain_id(chain_id);

    let client = SignerMiddleware::new(provider, wallet);

    let tx = TransactionRequest::pay(address, U256::zero());
    let receipt = client.send_transaction(tx, None).await?.await?;

    let hash = receipt.map(|r| r.transaction_hash);
    Ok(hash)
}

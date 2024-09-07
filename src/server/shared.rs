use std::convert::Infallible;

use ethers::prelude::{abigen, k256::ecdsa::SigningKey, Http, Provider, SignerMiddleware, Wallet};
use serde::{Deserialize, Serialize};
use warp::{http::StatusCode, Filter, Rejection, Reply};

abigen!(
    FaucetContract,
    r#"[{"name": "drip","type": "function","inputs": [{"name": "recipient","type": "address","internalType": "address payable"}],"outputs": [],"stateMutability": "nonpayable"}]"#
);

pub type DefaultSignerMiddleware = SignerMiddleware<Provider<Http>, Wallet<SigningKey>>;
pub type Faucet = FaucetContract<DefaultSignerMiddleware>;

/// Register request.
#[derive(Deserialize)]
pub struct RegisterRequest {
    /// The address to register.
    pub address: String,
    /// Whether to wait for the transaction to complete.
    /// Default is true.
    pub wait: Option<bool>,
}

impl std::fmt::Display for RegisterRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "address: {}", self.address)
    }
}

/// Generic request error.
#[derive(Clone, Debug)]
pub struct BadRequest {
    pub message: String,
}

impl warp::reject::Reject for BadRequest {}

/// Too many requests error.
#[derive(Clone, Debug)]
pub struct TooManyRequests {}

impl warp::reject::Reject for TooManyRequests {}

/// Faucet empty error.
#[derive(Clone, Debug)]
pub struct FaucetEmpty {}

impl warp::reject::Reject for FaucetEmpty {}

/// Custom error message with status code.
#[derive(Clone, Debug, Serialize)]
struct ErrorMessage {
    code: u16,
    message: String,
}

/// Rejection handler.
pub async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let (code, message) = if err.is_not_found() {
        (StatusCode::NOT_FOUND, "Not Found".to_string())
    } else if let Some(e) = err.find::<BadRequest>() {
        (StatusCode::BAD_REQUEST, e.message.clone())
    } else if let Some(_) = err.find::<TooManyRequests>() {
        (
            StatusCode::TOO_MANY_REQUESTS,
            "too many requests".to_string(),
        )
    } else if let Some(_) = err.find::<FaucetEmpty>() {
        (StatusCode::SERVICE_UNAVAILABLE, "faucet empty".to_string())
    } else if let Some(e) = err.find::<warp::filters::body::BodyDeserializeError>() {
        (
            StatusCode::BAD_REQUEST,
            format!("Invalid Request Body: {}", e),
        )
    } else if err.find::<warp::reject::InvalidHeader>().is_some() {
        (StatusCode::BAD_REQUEST, "Invalid Header Value".to_string())
    } else if err.find::<warp::reject::MethodNotAllowed>().is_some() {
        (
            StatusCode::METHOD_NOT_ALLOWED,
            "Method Not Allowed".to_string(),
        )
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error".to_string(),
        )
    };

    let reply = warp::reply::json(&ErrorMessage {
        code: code.as_u16(),
        message,
    });
    Ok(warp::reply::with_status(reply, code))
}

/// Filter to pass the faucet to the request handler.
pub fn with_faucet(faucet: Faucet) -> impl Filter<Extract = (Faucet,), Error = Infallible> + Clone {
    warp::any().map(move || faucet.clone())
}

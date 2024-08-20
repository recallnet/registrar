use ethers::prelude::Address as EthAddress;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use warp::{http::StatusCode, Filter, Rejection, Reply};

/// Generic base request for all routes.
#[derive(Deserialize)]
pub struct BaseRequest {
    pub address: String,
}

impl std::fmt::Display for BaseRequest {
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

/// Filter to pass the private key to the request handler.
pub fn with_private_key(
    private_key: String,
) -> impl Filter<Extract = (String,), Error = Infallible> + Clone {
    warp::any().map(move || private_key.clone())
}

/// Filter to pass the token address to the request handler.
pub fn with_token_address(
    token_address: EthAddress,
) -> impl Filter<Extract = (EthAddress,), Error = Infallible> + Clone {
    warp::any().map(move || token_address.clone())
}

/// Filter to pass the EVM RPC URL to the request handler.
pub fn with_rpc_url(
    rpc_url: String,
) -> impl Filter<Extract = (String,), Error = Infallible> + Clone {
    warp::any().map(move || rpc_url.clone())
}

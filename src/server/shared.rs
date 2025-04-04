use async_trait::async_trait;
use cf_turnstile::TurnstileClient;
use ethers::middleware::{Middleware, MiddlewareError};
use ethers::prelude::transaction::eip2718::TypedTransaction;
use ethers::prelude::{
    abigen, k256::ecdsa::SigningKey, BlockId, Http, NonceManagerMiddleware, PendingTransaction,
    Provider, SignerMiddleware, Wallet,
};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::{Arc, Condvar, Mutex};
use thiserror::Error;
use warp::{http::StatusCode, Filter, Rejection, Reply};

abigen!(
    FaucetContract,
    r#"[{"name": "drip","type": "function","inputs": [{"name": "recipient","type": "address","internalType": "address payable"}, {"internalType":"string[]","name":"keys","type":"string[]"}],"outputs": [],"stateMutability": "nonpayable"}]"#
);

pub type DefaultSignerMiddleware = SerializingMiddleware<
    SignerMiddleware<NonceManagerMiddleware<Provider<Http>>, Wallet<SigningKey>>,
>;
pub type Faucet = FaucetContract<DefaultSignerMiddleware>;

/// Drip request.
#[derive(Deserialize)]
pub struct DripRequest {
    /// The address to send the drip to.
    pub address: String,
    /// The Cloudflare Turnstile response to validate.
    pub ts_response: String,
    /// Whether to wait for the transaction to complete.
    /// Default is true.
    pub wait: Option<bool>,
}

impl std::fmt::Display for DripRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "address: {}, ts_response: {}, wait: {}",
            self.address,
            self.ts_response,
            self.wait.unwrap_or(true)
        )
    }
}

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
        write!(
            f,
            "address: {}, wait: {}",
            self.address,
            self.wait.unwrap_or(true)
        )
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
        (StatusCode::NOT_FOUND, "not found".to_string())
    } else if let Some(e) = err.find::<BadRequest>() {
        (StatusCode::BAD_REQUEST, e.message.clone())
    } else if err.find::<TooManyRequests>().is_some() {
        (
            StatusCode::TOO_MANY_REQUESTS,
            "too many requests".to_string(),
        )
    } else if err.find::<FaucetEmpty>().is_some() {
        (StatusCode::SERVICE_UNAVAILABLE, "faucet empty".to_string())
    } else if let Some(e) = err.find::<warp::filters::body::BodyDeserializeError>() {
        (
            StatusCode::BAD_REQUEST,
            format!("invalid request body: {}", e),
        )
    } else if err.find::<warp::reject::InvalidHeader>().is_some() {
        (StatusCode::BAD_REQUEST, "invalid header value".to_string())
    } else if err.find::<warp::reject::MethodNotAllowed>().is_some() {
        (
            StatusCode::METHOD_NOT_ALLOWED,
            "method not allowed".to_string(),
        )
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal server error".to_string(),
        )
    };

    let reply = warp::reply::json(&ErrorMessage {
        code: code.as_u16(),
        message,
    });
    Ok(warp::reply::with_status(reply, code))
}

/// Filter to pass the client to the request handler.
pub fn with_client(
    client: Arc<DefaultSignerMiddleware>,
) -> impl Filter<Extract = (Arc<DefaultSignerMiddleware>,), Error = Infallible> + Clone {
    warp::any().map(move || client.clone())
}

/// Filter to pass the faucet to the request handler.
pub fn with_faucet(faucet: Faucet) -> impl Filter<Extract = (Faucet,), Error = Infallible> + Clone {
    warp::any().map(move || faucet.clone())
}

/// Filter to pass the Cloudflare Turnstile client to the request handler.
pub fn with_turnstile(
    turnstile: Arc<TurnstileClient>,
) -> impl Filter<Extract = (Arc<TurnstileClient>,), Error = Infallible> + Clone {
    warp::any().map(move || turnstile.clone())
}

/// Ethers middleware to serialize all transactions to avoid nonce ordering issues.
#[derive(Debug)]
pub struct SerializingMiddleware<M> {
    inner: M,
    txn_running: Mutex<bool>,
    condvar: Condvar,
}

impl<M> SerializingMiddleware<M>
where
    M: Middleware,
{
    pub fn new(inner: M) -> Self {
        Self {
            inner,
            txn_running: Mutex::new(false),
            condvar: Condvar::new(),
        }
    }

    /// Call before sending a txn.  Blocks until no other threads are in the process of sending a
    /// txn.  Must be paired with a corresponding call to txn_complete.
    pub async fn txn_start(&self) -> Result<(), SerializingMiddlewareError<M>> {
        let mut running = self
            .txn_running
            .lock()
            .map_err(|_| SerializingMiddlewareError::MutexLockError)?;
        while *running {
            running = self
                .condvar
                .wait(running)
                .map_err(|_| SerializingMiddlewareError::MutexLockError)?;
        }
        *running = true;
        Ok(())
    }

    /// Call after sending a txn.  Wakes up the next thread waiting to send a txn.  Must be called
    /// after a corresponding call to txn_begin.
    pub async fn txn_complete(&self) -> Result<(), SerializingMiddlewareError<M>> {
        let mut running = self
            .txn_running
            .lock()
            .map_err(|_| SerializingMiddlewareError::MutexLockError)?;
        *running = false;
        self.condvar.notify_one();
        Ok(())
    }
}

#[derive(Error, Debug)]
/// Thrown when an error happens in the SerializingMiddleware
pub enum SerializingMiddlewareError<M: Middleware> {
    #[error("Error locking mutex")]
    /// Thrown when the internal call to the signer fails
    MutexLockError,

    /// Thrown when the internal middleware errors
    #[error("{0}")]
    MiddlewareError(M::Error),

    #[error("Not Implemented")]
    NotImplemented,
}

impl<M: Middleware> MiddlewareError for SerializingMiddlewareError<M> {
    type Inner = M::Error;

    fn from_err(src: M::Error) -> Self {
        SerializingMiddlewareError::MiddlewareError(src)
    }

    fn as_inner(&self) -> Option<&Self::Inner> {
        match self {
            SerializingMiddlewareError::MiddlewareError(e) => Some(e),
            _ => None,
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl<M> Middleware for SerializingMiddleware<M>
where
    M: Middleware,
{
    type Error = SerializingMiddlewareError<M>;
    type Provider = M::Provider;
    type Inner = M;

    fn inner(&self) -> &M {
        &self.inner
    }

    async fn fill_transaction(
        &self,
        _: &mut TypedTransaction,
        _: Option<BlockId>,
    ) -> Result<(), Self::Error> {
        Err(Self::Error::NotImplemented)
    }

    /// Sends the transaction, while serializing txn sending with all other requests using the same
    /// Provider.
    async fn send_transaction<T: Into<TypedTransaction> + Send + Sync>(
        &self,
        tx: T,
        block: Option<BlockId>,
    ) -> Result<PendingTransaction<'_, Self::Provider>, Self::Error> {
        self.txn_start().await?;

        let result = self
            .inner
            .send_transaction(tx, block)
            .await
            .map_err(MiddlewareError::from_err);

        self.txn_complete().await?;

        result
    }
}

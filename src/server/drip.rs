use crate::server::{
    shared::{
        with_faucet, with_google_sheets_config, with_turnstile, BadRequest,
        DefaultSignerMiddleware, DripRequest, Faucet, FaucetEmpty, GoogleSheetsConfig,
        TooManyRequests,
    },
    util::log_request_body,
};
use anyhow::anyhow;
use cf_turnstile::{SiteVerifyRequest, TurnstileClient};
use ethers::prelude::{Address, ContractError, TxHash};
use ethers::utils::keccak256;
use log::info;
use once_cell::sync::Lazy;
use serde_json::json;
use std::net::IpAddr;
use std::sync::Arc;
use warp::{Filter, Rejection, Reply};
use warp_real_ip::real_ip;

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

/// Route filter for `/drip` endpoint.
pub fn drip_route(
    trusted_proxy_ips: Vec<IpAddr>,
    faucet: Faucet,
    turnstile: Arc<TurnstileClient>,
    config: Arc<GoogleSheetsConfig>,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path("drip")
        .and(warp::post())
        .and(warp::header::exact("content-type", "application/json"))
        .and(warp::body::json())
        .and(real_ip(trusted_proxy_ips))
        .and(with_faucet(faucet))
        .and(with_turnstile(turnstile))
        .and(with_google_sheets_config(config))
        .and_then(handle_drip)
}

/// Handles the `/drip` request.
pub async fn handle_drip(
    req: DripRequest,
    addr: Option<IpAddr>,
    faucet: Faucet,
    turnstile: Arc<TurnstileClient>,
    config: Arc<GoogleSheetsConfig>,
) -> anyhow::Result<impl Reply, Rejection> {
    log_request_body("drip", &format!("{}", req));

    let addr = addr.ok_or(Rejection::from(BadRequest {
        message: "could not resolve ip address".to_string(),
    }))?;

    let to_address = req.address.parse::<Address>().map_err(|e| {
        Rejection::from(BadRequest {
            message: format!("invalid ethereum address: {}", e),
        })
    })?;

    let is_allowed = check_address_in_sheet(
        &req.address,
        &config.google_sheets_api_key,
        &config.allowlist_spreadsheet_id,
    )
    .await
    .map_err(|e| {
        Rejection::from(BadRequest {
            message: format!("failed to check address in allowlist: {}", e),
        })
    })?;

    if !is_allowed {
        return Err(Rejection::from(BadRequest {
            message: "address not in allowlist".to_string(),
        }));
    }

    let validated = turnstile
        .siteverify(SiteVerifyRequest {
            response: req.ts_response,
            ..Default::default()
        })
        .await
        .map_err(|e| {
            Rejection::from(BadRequest {
                message: format!("turnstile error: {}", e),
            })
        })?;

    if !validated.success {
        return Err(Rejection::from(BadRequest {
            message: "turnstile validation failed".to_string(),
        }));
    }

    let ip_string = addr.to_string();

    info!(
        "Calling drip with keys: address: {}, ip: {}",
        req.address, ip_string
    );

    let res = drip(faucet, to_address, vec![req.address, ip_string], req.wait)
        .await
        .map_err(|e| {
            Rejection::from(BadRequest {
                message: format!("drip error: {}", e),
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

/// Checks if an address exists in the Google Sheet
async fn check_address_in_sheet(
    address: &str,
    api_key: &str,
    spreadsheet_id: &str,
) -> anyhow::Result<bool> {
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{}/values/Sheet1!A:A?key={}",
        spreadsheet_id, api_key
    );

    let response = reqwest::get(&url).await?;
    let data: serde_json::Value = response.json().await?;

    let empty_vec = Vec::new();
    let values = data
        .get("values")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty_vec);

    let address = address.to_lowercase();
    Ok(values.iter().any(|row| {
        row.get(0)
            .and_then(|cell| cell.as_str())
            .map(|cell| cell.to_lowercase() == address)
            .unwrap_or(false)
    }))
}

/// Drips a small amount of HOKU to an address on the subnet using the faucet.
/// This will trigger the FVM to create an account for the address.
async fn drip(
    faucet: Faucet,
    to_address: Address,
    keys: Vec<String>,
    wait: Option<bool>,
) -> anyhow::Result<DripResult> {
    let tx = faucet.drip(to_address, keys);
    let tx_pending = tx.send().await;
    match tx_pending {
        Ok(tx) => {
            let hash = tx.tx_hash();
            let wait = wait.unwrap_or(true);
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

use log::{error, info};
use serde_json::json;
use warp::log::Info;

/// Helper function to log details for failed requests.
pub fn log_failed_request(request: Info) {
    if request.status().as_u16() < 400 {
        return;
    }
    let addr = request
        .remote_addr()
        .unwrap_or_else(|| ([0, 0, 0, 0], 0).into());
    let duration = request.elapsed().as_millis();
    let log_data = json!({
        "method": request.method().as_str(),
        "path": request.path(),
        "status": request.status().as_u16(),
        "client_addr": addr,
        "duration_ms": duration
    });
    error!("{}", log_data)
}

/// Helper function to log the incoming request body for a route when
/// [`Level::Info`] logging is enabled.
pub fn log_request_body(route: &str, body: &str) {
    let log_data = json!({
        "route": route,
        "body": body
    });
    info!("{}", log_data);
}

use lazy_static::lazy_static;
use log::debug;
use prometheus::{register_histogram_vec, HistogramVec};
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
    debug!("{}", log_data)
}

/// Helper function to log the incoming request body for a route when
/// [`Level::Info`] logging is enabled.
pub fn log_request_body(route: &str, body: &str) {
    let log_data = json!({
        "route": route,
        "body": body
    });
    debug!("{}", log_data);
}

lazy_static! {
    static ref HISTOGRAM_REQUESTS: HistogramVec = register_histogram_vec!(
        "http_request_duration_seconds",
        "Duration of HTTP requests in seconds.",
        &["methon", "path", "status"]
    )
    .unwrap();
}

pub fn request_metrics(request: Info) {
    let normalized_path: String = request.path().chars().skip(1).take(8).collect();
    HISTOGRAM_REQUESTS
        .with_label_values(&[
            request.method().as_str(),
            &normalized_path,
            request.status().as_str(),
        ])
        .observe(request.elapsed().as_secs_f64());
}

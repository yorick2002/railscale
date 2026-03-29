use rail_carriage::http::run_http;

#[tokio::main]
pub async fn main() {
    run_http().await;
}
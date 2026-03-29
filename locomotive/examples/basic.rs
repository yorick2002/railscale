use bytes::Bytes;
use memchr::memmem::Finder;
use locomotive::{HttpParser, HttpPipeline, TcpDestination, TcpSource};
use opentelemetry::global;
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider};
use opentelemetry_stdout::MetricExporter;
use std::sync::Arc;
use std::time::Duration;
use train_track::{Pipeline, Service};
use train_track::sampler;

fn init_metrics() -> SdkMeterProvider {
    let exporter = MetricExporter::default();
    let reader = PeriodicReader::builder(exporter, opentelemetry_sdk::runtime::Tokio)
        .with_interval(Duration::from_secs(10))
        .build();
    let provider = SdkMeterProvider::builder()
        .with_reader(reader)
        .build();
    global::set_meter_provider(provider.clone());
    provider
}

#[tokio::main]
#[hotpath::main]
async fn main() {
    hotpath::tokio_runtime!();
    tracing_subscriber::fmt()
        .with_target(false)
        .with_env_filter("warn")
        .init();

    let _metrics = init_metrics();

    let metrics_path = std::env::var("RAILSCALE_METRICS_FILE")
        .unwrap_or_else(|_| "/tmp/railscale-metrics.jsonl".to_string());
    let sampler_handle = Arc::new(sampler::start_sampler(&metrics_path, Duration::from_millis(100)));

    let source = TcpSource::bind("127.0.0.1:8080").await.unwrap();

    let pipeline = Pipeline {
        source,
        parser_factory: || HttpParser::new(vec![]),
        pipeline: Arc::new(HttpPipeline::new(vec![
            (Finder::new(b"User-Agent"), Bytes::from_static(b"railscale/1.0")),
        ])),
        destination_factory: || TcpDestination::with_fixed_upstream("127.0.0.1:9090"),
        sampler: Some(sampler_handle),
    };

    pipeline.run().await.unwrap();
}

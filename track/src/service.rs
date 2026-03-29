use std::pin::pin;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;
use opentelemetry::global;
use opentelemetry::metrics::{Counter, Histogram, UpDownCounter};
use tokio_stream::StreamExt;
use tracing::warn;
use crate::destination::StreamDestination;
use crate::frame::{Frame, ParsedData};
use crate::parser::FrameParser;
use crate::pipeline::FramePipeline;
use crate::sampler::{RequestRecord, SamplerHandle};
use crate::source::StreamSource;
use crate::RailscaleError;

pub trait Service: Send + Sync {
    fn run(&self) -> impl std::future::Future<Output = Result<(), RailscaleError>> + Send;
}

struct OtelMetrics {
    connections_total: Counter<u64>,
    connections_active: UpDownCounter<i64>,
    connection_errors: Counter<u64>,
    connection_duration: Histogram<f64>,
    request_forward_duration: Histogram<f64>,
    upstream_connect_duration: Histogram<f64>,
    response_relay_duration: Histogram<f64>,
    response_bytes: Histogram<f64>,
    frames_parsed: Counter<u64>,
    bytes_passthrough: Counter<u64>,
}

impl OtelMetrics {
    fn new() -> Self {
        let meter = global::meter("railscale");
        let latency_buckets = vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 5.0, 10.0];
        let size_buckets = vec![64.0, 256.0, 1024.0, 4096.0, 16384.0, 65536.0, 262144.0, 1048576.0];

        Self {
            connections_total: meter.u64_counter("railscale.connections_total").build(),
            connections_active: meter.i64_up_down_counter("railscale.connections_active").build(),
            connection_errors: meter.u64_counter("railscale.connection_errors").build(),
            connection_duration: meter.f64_histogram("railscale.connection_duration_seconds")
                .with_boundaries(latency_buckets.clone()).build(),
            request_forward_duration: meter.f64_histogram("railscale.request_forward_duration_seconds")
                .with_boundaries(latency_buckets.clone()).build(),
            upstream_connect_duration: meter.f64_histogram("railscale.upstream_connect_duration_seconds")
                .with_boundaries(latency_buckets.clone()).build(),
            response_relay_duration: meter.f64_histogram("railscale.response_relay_duration_seconds")
                .with_boundaries(latency_buckets).build(),
            response_bytes: meter.f64_histogram("railscale.response_bytes")
                .with_boundaries(size_buckets).build(),
            frames_parsed: meter.u64_counter("railscale.frames_parsed").build(),
            bytes_passthrough: meter.u64_counter("railscale.bytes_passthrough").build(),
        }
    }
}

struct ConnectionResult {
    forward_duration: f64,
    connect_duration: f64,
    relay_duration: f64,
    frame_count: u64,
    response_bytes: u64,
    request_bytes: u64,
}

pub struct Pipeline<Src, Par, Pip, Dst>
where
    Src: StreamSource,
    Par: FrameParser<Src::ReadHalf>,
    Pip: FramePipeline<Frame = Par::Frame>,
    Dst: StreamDestination<Frame = Par::Frame>,
{
    pub source: Src,
    pub parser_factory: fn() -> Par,
    pub pipeline: Arc<Pip>,
    pub destination_factory: fn() -> Dst,
    pub sampler: Option<Arc<SamplerHandle>>,
}

impl<Src, Par, Pip, Dst> Pipeline<Src, Par, Pip, Dst>
where
    Src: StreamSource + Sync,
    Par: FrameParser<Src::ReadHalf> + 'static,
    Pip: FramePipeline<Frame = Par::Frame> + 'static,
    Dst: StreamDestination<Frame = Par::Frame> + 'static,
{
    async fn handle_connection(
        read_half: Src::ReadHalf,
        mut write_half: Src::WriteHalf,
        parser_factory: fn() -> Par,
        pipeline: Arc<Pip>,
        destination_factory: fn() -> Dst,
        otel: Arc<OtelMetrics>,
        sampler: Option<Arc<SamplerHandle>>,
        start_time: Instant,
    ) {
        if let Some(ref s) = sampler {
            s.shared().active_connections.fetch_add(1, Ordering::Relaxed);
        }
        otel.connections_active.add(1, &[]);
        otel.connections_total.add(1, &[]);

        let conn_start = Instant::now();
        let result = Self::do_connection(
            read_half, &mut write_half, parser_factory, pipeline, destination_factory, &otel,
        ).await;

        let total_duration = conn_start.elapsed().as_secs_f64();
        otel.connections_active.add(-1, &[]);
        otel.connection_duration.record(total_duration, &[]);

        if let Some(ref s) = sampler {
            s.shared().active_connections.fetch_add(-1, Ordering::Relaxed);
        }

        match result {
            Ok(cr) => {
                if let Some(s) = sampler {
                    s.log_request(RequestRecord {
                        t: start_time.elapsed().as_secs_f64(),
                        total_us: (total_duration * 1e6) as u64,
                        connect_us: (cr.connect_duration * 1e6) as u64,
                        forward_us: (cr.forward_duration * 1e6) as u64,
                        relay_us: (cr.relay_duration * 1e6) as u64,
                        frames: cr.frame_count,
                        req_bytes: cr.request_bytes,
                        resp_bytes: cr.response_bytes,
                        error: false,
                    });
                }
            }
            Err(e) => {
                otel.connection_errors.add(1, &[]);
                if let Some(s) = sampler {
                    s.log_request(RequestRecord {
                        t: start_time.elapsed().as_secs_f64(),
                        total_us: (total_duration * 1e6) as u64,
                        connect_us: 0,
                        forward_us: 0,
                        relay_us: 0,
                        frames: 0,
                        req_bytes: 0,
                        resp_bytes: 0,
                        error: true,
                    });
                }
                warn!(error = %e, "connection error");
            }
        }
    }

    async fn do_connection(
        read_half: Src::ReadHalf,
        write_half: &mut Src::WriteHalf,
        parser_factory: fn() -> Par,
        pipeline: Arc<Pip>,
        destination_factory: fn() -> Dst,
        otel: &OtelMetrics,
    ) -> Result<ConnectionResult, RailscaleError> {
        let forward_start = Instant::now();
        let mut parser = parser_factory();
        let mut dest = destination_factory();
        let frames = parser.parse(read_half);
        let mut frames = pin!(frames);
        let mut routed = false;
        let mut frame_count: u64 = 0;
        let mut passthrough_bytes: u64 = 0;
        let mut request_bytes: u64 = 0;
        let mut connect_duration: f64 = 0.0;

        while let Some(result) = frames.next().await {
            match result {
                Ok(ParsedData::Passthrough(bytes)) => {
                    passthrough_bytes += bytes.len() as u64;
                    request_bytes += bytes.len() as u64;
                    dest.write_raw(bytes).await.map_err(Into::into)?;
                }
                Ok(ParsedData::Parsed(frame)) => {
                    frame_count += 1;
                    request_bytes += frame.as_bytes().len() as u64;
                    if frame.is_routing_frame() && !routed {
                        let connect_start = Instant::now();
                        dest.provide(&frame).await.map_err(Into::into)?;
                        connect_duration = connect_start.elapsed().as_secs_f64();
                        otel.upstream_connect_duration.record(connect_duration, &[]);
                        routed = true;
                    }
                    let frame = pipeline.process(frame);
                    dest.write(frame).await.map_err(Into::into)?;
                }
                Err(e) => {
                    warn!(error = %e.into(), "frame parse error");
                    return Err(RailscaleError::ConnectionClosed);
                }
            }
        }

        let forward_duration = forward_start.elapsed().as_secs_f64();
        otel.request_forward_duration.record(forward_duration, &[]);
        otel.frames_parsed.add(frame_count, &[]);
        otel.bytes_passthrough.add(passthrough_bytes, &[]);

        let relay_start = Instant::now();
        let response_bytes = dest.relay_response(write_half).await.map_err(Into::into)?;
        let relay_duration = relay_start.elapsed().as_secs_f64();
        otel.response_relay_duration.record(relay_duration, &[]);
        otel.response_bytes.record(response_bytes as f64, &[]);

        Ok(ConnectionResult {
            forward_duration,
            connect_duration,
            relay_duration,
            frame_count,
            response_bytes,
            request_bytes,
        })
    }
}

impl<Src, Par, Pip, Dst> Service for Pipeline<Src, Par, Pip, Dst>
where
    Src: StreamSource + Sync + 'static,
    Src::ReadHalf: Send + 'static,
    Src::WriteHalf: Send + 'static,
    Par: FrameParser<Src::ReadHalf> + 'static,
    Par::Error: Send,
    Pip: FramePipeline<Frame = Par::Frame> + 'static,
    Dst: StreamDestination<Frame = Par::Frame> + 'static,
{
    async fn run(&self) -> Result<(), RailscaleError> {
        let otel = Arc::new(OtelMetrics::new());
        let sampler = self.sampler.clone();
        let start_time = Instant::now();

        loop {
            let (read_half, write_half) = self.source.accept().await.map_err(Into::into)?;
            let parser_factory = self.parser_factory;
            let destination_factory = self.destination_factory;
            let pipeline = Arc::clone(&self.pipeline);
            let otel = Arc::clone(&otel);
            let sampler = sampler.clone();

            tokio::spawn(Self::handle_connection(
                read_half, write_half, parser_factory, pipeline, destination_factory,
                otel, sampler, start_time,
            ));
        }
    }
}

#!/usr/bin/env bash
set -euo pipefail

TARGET="${1:-http://127.0.0.1:8080}"
RATE="${2:-1000}"
DURATION="${3:-10s}"
OUT_DIR="loadtest-results"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

mkdir -p "$OUT_DIR"

if ! command -v vegeta &>/dev/null; then
    echo "vegeta not found — install: brew install vegeta"
    exit 1
fi

TIMESTAMP=$(date +%Y%m%d_%H%M%S)
RESULTS="$OUT_DIR/$TIMESTAMP"
mkdir -p "$RESULTS"

METRICS_FILE="${RAILSCALE_METRICS_FILE:-/tmp/railscale-metrics.jsonl}"

echo "=== Railscale Load Test ==="
echo "  target:   $TARGET"
echo "  rate:     $RATE/s"
echo "  duration: $DURATION"
echo "  output:   $RESULTS/"
echo ""

echo "GET $TARGET" \
    | vegeta attack -rate="$RATE/s" -duration="$DURATION" \
    | tee "$RESULTS/results.bin" \
    | vegeta encode --to json > "$RESULTS/results.json"

vegeta report -type=text "$RESULTS/results.bin" > "$RESULTS/report.txt"
vegeta report "$RESULTS/results.bin"
echo ""

sleep 1

REQUEST_FILE="${METRICS_FILE%.jsonl}-requests.jsonl"

if [ -f "$METRICS_FILE" ] && [ -s "$METRICS_FILE" ]; then
    cp "$METRICS_FILE" "$RESULTS/railscale-metrics.jsonl"
    [ -f "$REQUEST_FILE" ] && cp "$REQUEST_FILE" "$RESULTS/railscale-metrics-requests.jsonl"
    METRIC_LINES=$(wc -l < "$METRICS_FILE" | tr -d ' ')
    REQ_LINES=$( [ -f "$REQUEST_FILE" ] && wc -l < "$REQUEST_FILE" | tr -d ' ' || echo 0 )
    echo "Proxy metrics: $METRIC_LINES system samples, $REQ_LINES request records"
    CHART_METRICS="$RESULTS/railscale-metrics.jsonl"
else
    echo "No proxy metrics found at $METRICS_FILE"
    echo "  Start proxy with: RAILSCALE_METRICS_FILE=$METRICS_FILE cargo run --example basic"
    touch "$RESULTS/railscale-metrics.jsonl"
    CHART_METRICS="$RESULTS/railscale-metrics.jsonl"
fi

echo "Generating chart..."
python3 "$SCRIPT_DIR/gen-chart.py" "$RESULTS/results.json" "$CHART_METRICS" "$RESULTS/chart.html"

echo ""
echo "results:  $RESULTS/results.bin"
echo "report:   $RESULTS/report.txt"
echo "metrics:  $RESULTS/railscale-metrics.jsonl"
echo "chart:    $RESULTS/chart.html"
echo ""
echo "open $RESULTS/chart.html"

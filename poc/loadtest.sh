#!/usr/bin/env bash
set -euo pipefail

TARGET="${1:-http://127.0.0.1:8080}"
RATE="${2:-1000}"
DURATION="${3:-10s}"
OUT_DIR="loadtest-results"

mkdir -p "$OUT_DIR"

if ! command -v vegeta &>/dev/null; then
    echo "vegeta not found — install: brew install vegeta"
    exit 1
fi

TIMESTAMP=$(date +%Y%m%d_%H%M%S)
RESULTS="$OUT_DIR/$TIMESTAMP"
mkdir -p "$RESULTS"

echo "GET $TARGET" \
    | vegeta attack -rate="$RATE/s" -duration="$DURATION" \
    | tee "$RESULTS/results.bin" \
    | vegeta encode --to json > "$RESULTS/results.json"

vegeta report "$RESULTS/results.bin"

vegeta report -type=text "$RESULTS/results.bin" > "$RESULTS/report.txt"

vegeta plot "$RESULTS/results.bin" > "$RESULTS/plot.html"

echo ""
echo "results:  $RESULTS/results.bin"
echo "report:   $RESULTS/report.txt"
echo "chart:    $RESULTS/plot.html"
echo ""
echo "open $RESULTS/plot.html"

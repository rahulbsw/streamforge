#!/bin/bash

# WAP MirrorMaker Benchmarking Script
# Runs performance benchmarks and generates reports

set -e

echo "========================================="
echo "WAP MirrorMaker Performance Benchmarks"
echo "========================================="
echo ""

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Check if cargo is installed
if ! command -v cargo &> /dev/null; then
    echo "Error: cargo not found. Please install Rust."
    exit 1
fi

echo -e "${BLUE}Step 1: Building benchmarks...${NC}"
cargo build --release --benches

echo ""
echo -e "${BLUE}Step 2: Running filter benchmarks...${NC}"
cargo bench --bench filter_benchmarks

echo ""
echo -e "${BLUE}Step 3: Running transform benchmarks...${NC}"
cargo bench --bench transform_benchmarks

echo ""
echo -e "${GREEN}✓ Benchmarks complete!${NC}"
echo ""
echo "Results saved to:"
echo "  - target/criterion/filter/"
echo "  - target/criterion/transform/"
echo ""
echo "To view HTML reports:"
echo "  open target/criterion/report/index.html"
echo ""
echo "To run specific benchmarks:"
echo "  cargo bench filter/simple_numeric_gt"
echo "  cargo bench transform/construct_small"
echo ""
echo "To compare against baseline:"
echo "  cargo bench --bench filter_benchmarks -- --save-baseline main"
echo "  # Make changes..."
echo "  cargo bench --bench filter_benchmarks -- --baseline main"
echo ""

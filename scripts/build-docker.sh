#!/bin/bash
set -e

echo "🐳 Building WAP MirrorMaker Docker Images"
echo "========================================="

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Build dynamic version
echo -e "\n${BLUE}Building dynamic version...${NC}"
docker build -t wap-mirrormaker-rust:latest -t wap-mirrormaker-rust:dynamic .

echo -e "${GREEN}✓ Dynamic build complete${NC}"
docker images wap-mirrormaker-rust:latest

# Build static version
echo -e "\n${BLUE}Building static version...${NC}"
docker build -f Dockerfile.static -t wap-mirrormaker-rust:static .

echo -e "${GREEN}✓ Static build complete${NC}"
docker images wap-mirrormaker-rust:static

# Show sizes
echo -e "\n${BLUE}Image sizes:${NC}"
docker images wap-mirrormaker-rust --format "table {{.Repository}}\t{{.Tag}}\t{{.Size}}"

echo -e "\n${GREEN}✓ All images built successfully!${NC}"
echo ""
echo "Run with:"
echo "  docker run -v \$(pwd)/config.json:/app/config/config.json:ro wap-mirrormaker-rust:latest"
echo ""
echo "Or with example config:"
echo "  docker run -v \$(pwd)/examples/config.example.yaml:/app/config/config.yaml:ro wap-mirrormaker-rust:latest"
echo ""
echo "Or use docker-compose:"
echo "  docker-compose up -d"

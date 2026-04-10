#!/bin/bash

# Bundle script for open2api deployment
# Creates a self-contained distribution directory

set -e

# Configuration
OUTPUT_DIR="${1:-dist}"
EXECUTABLE="target/release/open2api"
ENV_FILE=".env"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color

echo "Creating deployment bundle in: $OUTPUT_DIR"

# Check if executable exists
if [ ! -f "$EXECUTABLE" ]; then
    echo -e "${RED}Error: Executable not found at $EXECUTABLE${NC}"
    echo "Run 'cargo build --release' first"
    exit 1
fi

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Copy executable
cp "$EXECUTABLE" "$OUTPUT_DIR/"
echo -e "${GREEN}✓${NC} Copied executable"

# Copy .env if it exists
if [ -f "$ENV_FILE" ]; then
    cp "$ENV_FILE" "$OUTPUT_DIR/"
    echo -e "${GREEN}✓${NC} Copied .env"
else
    echo -e "${RED}Warning: .env file not found${NC}"
fi

# Copy .env.example
if [ -f ".env.example" ]; then
    cp ".env.example" "$OUTPUT_DIR/"
    echo -e "${GREEN}✓${NC} Copied .env.example"
fi

# Set executable permissions
chmod +x "$OUTPUT_DIR/open2api"

echo ""
echo -e "${GREEN}Bundle created successfully!${NC}"
echo ""
echo "Directory structure:"
echo "  $OUTPUT_DIR/"
echo "  ├── open2api"
echo "  ├── .env"
echo "  └── .env.example"
echo ""
echo "To run:"
echo "  cd $OUTPUT_DIR && ./open2api"
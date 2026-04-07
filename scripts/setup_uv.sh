#!/bin/bash
# ============================================================
# UV ENVIRONMENT SETUP SCRIPT
# ============================================================

set -euo pipefail

echo "Setting up HFT Stealth System with UV..."

# Install UV if not present
if ! command -v uv &> /dev/null; then
    echo "Installing UV..."
    curl -LsSf https://astral.sh/uv/install.sh | sh
    export PATH="$HOME/.cargo/bin:$PATH"
fi

# Create virtual environment
echo "Creating virtual environment..."
uv venv --python 3.11

# Activate virtual environment
source .venv/bin/activate

# Install Python dependencies
echo "Installing Python dependencies..."
uv pip install -e ".[dev,monitoring,backtest]"

# Install maturin for Rust bindings
echo "Installing maturin..."
uv pip install maturin

# Build Rust bindings
echo "Building Rust Python bindings..."
maturin develop --release

# Install pre-commit hooks
if command -v pre-commit &> /dev/null; then
    pre-commit install
fi

echo ""
echo "Environment setup complete!"
echo ""
echo "To activate: source .venv/bin/activate"
echo "To run: make dev"
echo "To test: make test"

# ============================================================
# HFT STEALTH SYSTEM MAKEFILE (OPTIMIZED)
# ============================================================

SHELL := /bin/bash
.ONESHELL:
.SHELLFLAGS := -eu -o pipefail -c
.DELETE_ON_ERROR:
MAKEFLAGS += --jobs=$(shell nproc)

# ============================================================
# CONFIGURATION
# ============================================================

CARGO := cargo
UV := uv
RUSTC_FLAGS := -C target-cpu=native -C opt-level=3 -C lto=fat
CARGO_FLAGS := --release --features production

# Detect OS and architecture
UNAME_S := $(shell uname -s)
UNAME_P := $(shell uname -m)

# Performance tuning
ifeq ($(UNAME_S),Linux)
    NUMA_AVAILABLE := $(shell command -v numactl 2>/dev/null && echo "yes" || echo "no")
    ifeq ($(NUMA_AVAILABLE),yes)
        NUMA_PREFIX := numactl --cpunodebind=0 --membind=0
    endif
endif

# ============================================================
# PHONY TARGETS
# ============================================================

.PHONY: help setup build build-fast run run-prod test bench clean \
        dev prod dashboard health check lint fmt doc \
        perf flamegraph strace precommit

# ============================================================
# HELP
# ============================================================

help:
	@echo "╔══════════════════════════════════════════════════════════════╗"
	@echo "║              HFT STEALTH SYSTEM - MAKE COMMANDS              ║"
	@echo "╚══════════════════════════════════════════════════════════════╝"
	@echo ""
	@echo "  Setup & Build:"
	@echo "  make setup      - Setup UV environment and dependencies"
	@echo "  make build      - Build optimized release binary"
	@echo "  make build-fast - Fast build (no LTO, for testing)"
	@echo ""
	@echo "  Run:"
	@echo "  make run        - Run development version"
	@echo "  make run-prod   - Run production version (with NUMA binding)"
	@echo "  make dev        - Start full dev environment"
	@echo "  make prod       - Start full production environment"
	@echo ""
	@echo "  Test & Benchmark:"
	@echo "  make test       - Run all tests"
	@echo "  make bench      - Run performance benchmarks"
	@echo "  make perf       - Run perf profiling"
	@echo "  make flamegraph - Generate flamegraph"
	@echo ""
	@echo "  Monitoring:"
	@echo "  make dashboard  - Start web dashboard"
	@echo "  make health     - Run health check"
	@echo "  make metrics    - Show live metrics"
	@echo ""
	@echo "  Quality:"
	@echo "  make lint       - Run linters"
	@echo "  make fmt        - Format code"
	@echo "  make doc        - Generate documentation"
	@echo ""
	@echo "  Cleanup:"
	@echo "  make clean      - Clean all build artifacts"
	@echo "  make clean-cache - Clean cargo/uv caches"

# ============================================================
# SETUP
# ============================================================

setup:
	@echo "🔧 Setting up HFT Stealth System..."
	@command -v uv >/dev/null 2>&1 || { \
		echo "Installing UV..."; \
		curl -LsSf https://astral.sh/uv/install.sh | sh; \
	}
	@uv venv --python 3.11 .venv
	@.venv/bin/uv pip install -e ".[dev,monitoring]"
	@.venv/bin/uv pip install maturin
	@echo "✅ Setup complete. Run 'source .venv/bin/activate' to activate"

# ============================================================
# BUILD
# ============================================================

build:
	@echo "Building production binary..."
	@RUSTFLAGS="$(RUSTC_FLAGS)" $(CARGO) build $(CARGO_FLAGS)
	@echo "✅ Binary: target/release/hft_stealth_system"
	@ls -lh target/release/hft_stealth_system

build-fast:
	@echo "Building fast (debug) binary..."
	@$(CARGO) build
	@echo "✅ Binary: target/debug/hft_stealth_system"

# ============================================================
# RUN
# ============================================================

run: build-fast
	@echo "Running development version..."
	@export HFT_ENVIRONMENT=development
	@export RUST_LOG=debug
	@./scripts/run_prod.sh start --env development

run-prod: build
	@echo "Running production version..."
	@export HFT_ENVIRONMENT=production
	@export RUST_LOG=warn
	@$(NUMA_PREFIX) ./scripts/run_prod.sh start --env production

dev: setup
	@echo "Starting development environment..."
	@make -j2 run dashboard

prod: build
	@echo "Starting production environment..."
	@./scripts/run_prod.sh start
	@sleep 2
	@make dashboard &
	@echo "✅ System running. Dashboard: http://localhost:8080"

# ============================================================
# TESTING
# ============================================================

test:
	@echo "Running tests..."
	@$(CARGO) test --release -- --nocapture
	@.venv/bin/pytest tests/python/ -v --tb=short

test-quick:
	@echo "Running quick tests (no Python)..."
	@$(CARGO) test --release -- --nocapture --skip python

bench:
	@echo "Running benchmarks..."
	@./scripts/benchmark.sh all

perf:
	@echo "Running perf profiling..."
	@sudo perf record -g --call-graph dwarf target/release/hft_stealth_system --bench
	@sudo perf report

flamegraph:
	@echo "Generating flamegraph..."
	@cargo flamegraph -- --bench
	@echo "✅ Flamegraph: flamegraph.svg"

strace:
	@echo "Tracing system calls..."
	@sudo strace -c -f -e trace=network,file target/release/hft_stealth_system --bench

# ============================================================
# MONITORING
# ============================================================

dashboard:
	@echo "Starting monitoring dashboard..."
	@.venv/bin/python scripts/monitoring_dashboard.py --port 8080

health:
	@echo "Running health check..."
	@.venv/bin/python scripts/health_check.py --verbose

metrics:
	@echo "Live metrics (Ctrl+C to stop)..."
	@watch -n 1 'curl -s http://localhost:9090/metrics | grep -E "hft_(latency|throughput|detection)"'

# ============================================================
# QUALITY
# ============================================================

lint:
	@echo "Running linters..."
	@$(CARGO) clippy -- -D warnings
	@.venv/bin/ruff check python/
	@.venv/bin/mypy python/ --ignore-missing-imports

fmt:
	@echo " Formatting code..."
	@$(CARGO) fmt
	@.venv/bin/black python/
	@.venv/bin/ruff check --fix python/

doc:
	@echo " Generating documentation..."
	@$(CARGO) doc --no-deps --open

precommit:
	@echo " Setting up pre-commit hooks..."
	@.venv/bin/pre-commit install
	@echo "✅ Pre-commit hooks installed"

# ============================================================
# CLEANUP
# ============================================================

clean:
	@echo "Cleaning build artifacts..."
	@$(CARGO) clean
	@rm -rf target/
	@rm -rf .venv/
	@rm -rf python/__pycache__/
	@rm -rf tests/python/__pycache__/
	@rm -rf flamegraph.svg
	@echo "✅ Clean complete"

clean-cache:
	@echo " Cleaning caches..."
	@$(CARGO) cache -a 2>/dev/null || true
	@rm -rf ~/.cargo/registry/cache/
	@uv cache clean
	@echo "✅ Cache cleaned"

# ============================================================
# UTILITIES
# ============================================================

version:
	@echo "HFT Stealth System v1.0.0"
	@echo "Rust: $(shell rustc --version)"
	@echo "Cargo: $(shell cargo --version)"
	@echo "UV: $(shell uv --version 2>/dev/null || echo 'not installed')"

info:
	@echo "System Information:"
	@echo "  OS: $(UNAME_S)"
	@echo "  Arch: $(UNAME_P)"
	@echo "  CPUs: $(shell nproc)"
	@echo "  Memory: $(shell free -h | grep Mem | awk '{print $$2}')"
	@echo "  NUMA: $(NUMA_AVAILABLE)"

# ============================================================
# DEFAULT TARGET
# ============================================================

.DEFAULT_GOAL := help

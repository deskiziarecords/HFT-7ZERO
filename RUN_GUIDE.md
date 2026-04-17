# HFT-7ZERO: Operation & Deployment Guide

This guide provides step-by-step instructions on how to build, run, and maintain the HFT-7ZERO system, including the backend trading engine and the real-time monitoring dashboard.

---

## 1. Prerequisites

### Backend (Rust)
- **Rust Toolchain**: `rustc 1.77.0-nightly (2024-01-14)` is required.
- **System Dependencies**:
  - `numactl` (for NUMA binding)
  - `libssl-dev`
  - `build-essential`
  - `cmake` (for certain sub-dependencies)

### Frontend (Dashboard)
- **Node.js**: v18.0.0 or higher
- **npm**: v9.0.0 or higher

---

## 2. Backend Setup & Run

### 2.1 Building the System
For development:
```bash
cargo build
```

For production (with optimizations):
```bash
cargo build --profile production --features production
```

### 2.2 Running Tests
Execute the full test suite (unit and integration tests):
```bash
cargo test
```

To run a specific integration test (e.g., Hyperion Integration):
```bash
cargo test --test hyperion_integration_test
```

### 2.3 Running in Production
The system uses a dedicated production script located in `scripts/run_prod.sh`.

**First-time setup (Directories and Systemd):**
```bash
sudo ./scripts/run_prod.sh setup
```

**Start the system:**
```bash
sudo ./scripts/run_prod.sh start
```

**Monitor status and logs:**
```bash
./scripts/run_prod.sh status
./scripts/run_prod.sh logs
```

**Stop the system:**
```bash
sudo ./scripts/run_prod.sh stop
```

---

## 3. Frontend Setup & Run

The monitoring dashboard is a high-performance React application located in the `dashboard/` directory.

### 3.1 Installation
Navigate to the dashboard directory and install dependencies:
```bash
cd dashboard
npm install
```

### 3.2 Development Mode
Start the Vite development server:
```bash
npm run dev
```
The dashboard will be available at `http://localhost:5173` (or the port specified in the console).

### 3.3 Production Build
Build the optimized static assets:
```bash
npm run build
```
The assets will be generated in `dashboard/dist/`.

---

## 4. Automation Scripts

### 4.1 Benchmarking
Run the latency and throughput benchmarks:
```bash
./scripts/benchmark.sh all
```

### 4.2 Health Monitoring
A Python-based health checker is available:
```bash
# Ensure uv or pip is installed
python3 scripts/health_check.py
```

### 4.3 Deployment
To deploy the system to a remote host (configured in the script):
```bash
./scripts/deploy.sh
```

---

## 5. System Tuning (Production)
For optimal performance (<1ms latency), ensure the following:
1. **CPU Isolation**: Isolate cores specified in `CPU_AFFINITY` in `run_prod.sh`.
2. **Hugepages**: The `run_prod.sh` script attempts to configure 1024 hugepages. Ensure your system supports them.
3. **Memory Locking**: Ensure the user running the process has permissions for `ulimit -l unlimited`.

---
*Confidential - Internal Operational Document*

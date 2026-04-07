#!/bin/bash
# ============================================================
# PRODUCTION RUN SCRIPT
# ============================================================
# Starts the HFT stealth system in production mode
# Handles process management, logging, and monitoring
# ============================================================

set -euo pipefail

# ============================================================
# CONFIGURATION
# ============================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BINARY="${PROJECT_ROOT}/target/production/hft_stealth_system"
CONFIG_FILE="${PROJECT_ROOT}/config/production.toml"
PID_FILE="/var/run/hft-stealth.pid"
LOG_DIR="/var/log/hft-stealth"
DATA_DIR="/var/lib/hft-stealth"

# Performance tuning
CPU_AFFINITY="0,1,2,3"
NUMA_NODE="0"
HUGEPAGES=1024

# Network
INTERFACE="eth0"
MARKET_DATA_PORT=10000
ORDER_ENTRY_PORT=20000

# ============================================================
# ENVIRONMENT SETUP
# ============================================================

export HFT_ENVIRONMENT="production"
export HFT_LATENCY_BUDGET_US=1000
export HFT_STEALTH_ENABLED="true"
export HFT_DRY_RUN="false"
export RUST_BACKTRACE="1"
export RUST_LOG="info,hft_stealth_system=warn"

# Set CPU affinity
if [[ -n "$CPU_AFFINITY" ]]; then
    export HFT_CPU_AFFINITY="$CPU_AFFINITY"
fi

# Set NUMA node
if [[ -n "$NUMA_NODE" ]]; then
    export HFT_NUMA_NODE="$NUMA_NODE"
    # Bind to specific NUMA node
    numactl --cpunodebind="$NUMA_NODE" --membind="$NUMA_NODE" true || true
fi

# ============================================================
# FUNCTIONS
# ============================================================

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $1" | tee -a "${LOG_DIR}/run.log"
}

error() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] ERROR: $1" | tee -a "${LOG_DIR}/error.log" >&2
}

setup_directories() {
    mkdir -p "$LOG_DIR" "$DATA_DIR"
    chmod 750 "$LOG_DIR" "$DATA_DIR"
}

check_prerequisites() {
    # Check if binary exists
    if [[ ! -f "$BINARY" ]]; then
        error "Binary not found: $BINARY"
        error "Run 'cargo build --profile production' first"
        exit 1
    fi
    
    # Check if config exists
    if [[ ! -f "$CONFIG_FILE" ]]; then
        error "Config not found: $CONFIG_FILE"
        exit 1
    fi
    
    # Check if already running
    if [[ -f "$PID_FILE" ]] && kill -0 "$(cat "$PID_FILE")" 2>/dev/null; then
        error "System already running with PID $(cat "$PID_FILE")"
        exit 1
    fi
    
    # Check network interface
    if ! ip link show "$INTERFACE" >/dev/null 2>&1; then
        error "Network interface $INTERFACE not found"
        exit 1
    fi
    
    # Configure hugepages
    if [[ -f "/proc/sys/vm/nr_hugepages" ]]; then
        echo "$HUGEPAGES" > /proc/sys/vm/nr_hugepages 2>/dev/null || true
    fi
    
    # Set ulimits
    ulimit -n 65536
    ulimit -l unlimited  # Lock memory
    ulimit -s 8192       # Stack size
}

start_system() {
    log "Starting HFT Stealth System..."
    log "Binary: $BINARY"
    log "Config: $CONFIG_FILE"
    log "Log dir: $LOG_DIR"
    
    # Start with numactl if available
    if command -v numactl >/dev/null 2>&1 && [[ -n "$NUMA_NODE" ]]; then
        numactl --cpunodebind="$NUMA_NODE" --membind="$NUMA_NODE" \
            "$BINARY" --config "$CONFIG_FILE" \
            >> "${LOG_DIR}/stdout.log" 2>> "${LOG_DIR}/stderr.log" &
    else
        "$BINARY" --config "$CONFIG_FILE" \
            >> "${LOG_DIR}/stdout.log" 2>> "${LOG_DIR}/stderr.log" &
    fi
    
    PID=$!
    echo $PID > "$PID_FILE"
    
    log "Started with PID: $PID"
    
    # Wait for system to start
    sleep 2
    
    if kill -0 $PID 2>/dev/null; then
        log "System started successfully"
        return 0
    else
        error "System failed to start"
        tail -20 "${LOG_DIR}/stderr.log"
        return 1
    fi
}

setup_logrotate() {
    cat > /etc/logrotate.d/hft-stealth << EOF
${LOG_DIR}/*.log {
    daily
    rotate 7
    compress
    delaycompress
    missingok
    notifempty
    create 640 root root
    sharedscripts
    postrotate
        kill -HUP $(cat $PID_FILE 2>/dev/null) || true
    endscript
}
EOF
    log "Logrotate configured"
}

setup_systemd() {
    cat > /etc/systemd/system/hft-stealth.service << EOF
[Unit]
Description=HFT Stealth System
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=${PROJECT_ROOT}
ExecStart=${SCRIPT_DIR}/run_prod.sh start
ExecStop=${SCRIPT_DIR}/run_prod.sh stop
Restart=on-failure
RestartSec=5
LimitNOFILE=65536
LimitMEMLOCK=infinity
CPUSchedulingPolicy=fifo
CPUSchedulingPriority=99
Nice=-20

[Install]
WantedBy=multi-user.target
EOF
    systemctl daemon-reload
    log "Systemd service configured"
}

stop_system() {
    if [[ -f "$PID_FILE" ]]; then
        PID=$(cat "$PID_FILE")
        log "Stopping system with PID: $PID"
        
        # Send SIGTERM for graceful shutdown
        kill -TERM "$PID" 2>/dev/null || true
        
        # Wait up to 10 seconds for graceful shutdown
        for i in {1..10}; do
            if ! kill -0 "$PID" 2>/dev/null; then
                log "System stopped gracefully"
                rm -f "$PID_FILE"
                return 0
            fi
            sleep 1
        done
        
        # Force kill if still running
        kill -KILL "$PID" 2>/dev/null || true
        rm -f "$PID_FILE"
        log "System force stopped"
    else
        log "PID file not found, system not running"
    fi
}

status_system() {
    if [[ -f "$PID_FILE" ]] && kill -0 "$(cat "$PID_FILE")" 2>/dev/null; then
        PID=$(cat "$PID_FILE")
        echo "System running with PID: $PID"
        
        # Show resource usage
        if command -v ps >/dev/null 2>&1; then
            ps -p "$PID" -o pid,ppid,pcpu,pmem,rss,vsz,etime,cmd --no-headers 2>/dev/null || true
        fi
        
        # Show metrics endpoint if available
        if command -v curl >/dev/null 2>&1; then
            echo ""
            echo "Metrics:"
            curl -s "http://localhost:9090/metrics" 2>/dev/null | head -20 || echo "Metrics endpoint not available"
        fi
    else
        echo "System not running"
    fi
}

tail_logs() {
    tail -f "${LOG_DIR}/stdout.log" "${LOG_DIR}/stderr.log"
}

# ============================================================
# MAIN
# ============================================================

case "${1:-}" in
    start)
        setup_directories
        check_prerequisites
        start_system
        ;;
    stop)
        stop_system
        ;;
    restart)
        stop_system
        sleep 2
        start_system
        ;;
    status)
        status_system
        ;;
    logs)
        tail_logs
        ;;
    setup)
        setup_directories
        setup_logrotate
        setup_systemd
        ;;
    *)
        echo "Usage: $0 {start|stop|restart|status|logs|setup}"
        exit 1
        ;;
esac

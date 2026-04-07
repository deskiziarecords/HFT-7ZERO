#!/bin/bash
# ============================================================
# DEPLOYMENT SCRIPT
# ============================================================
# Deploys the HFT stealth system to production servers
# Handles blue-green deployment, rollback, and verification
# ============================================================

set -euo pipefail

# ============================================================
# CONFIGURATION
# ============================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Deployment targets
DEPLOY_USER="${DEPLOY_USER:-hft}"
DEPLOY_HOST="${DEPLOY_HOST:-prod-server-01}"
DEPLOY_PATH="/opt/hft-stealth"
BACKUP_PATH="/opt/hft-stealth-backups"

# Version info
VERSION=$(git describe --tags --always --dirty 2>/dev/null || echo "unknown")
BUILD_ID="${BUILD_ID:-$(date +%Y%m%d_%H%M%S)}"

# Deployment strategy
STRATEGY="${STRATEGY:-blue-green}"  # blue-green, rolling, canary
CANARY_PERCENT="${CANARY_PERCENT:-10}"
ROLLBACK_ENABLED="${ROLLBACK_ENABLED:-true}"

# Health check
HEALTH_CHECK_URL="http://localhost:9090/health"
HEALTH_CHECK_TIMEOUT=30
HEALTH_CHECK_RETRIES=3

# ============================================================
# FUNCTIONS
# ============================================================

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $1"
}

error() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] ERROR: $1" >&2
}

build() {
    log "Building production binary..."
    
    # Build with production profile
    cargo build --profile production --features production
    
    # Strip debug symbols
    strip target/production/hft_stealth_system
    
    log "Build complete: $(ls -lh target/production/hft_stealth_system)"
}

package() {
    log "Creating deployment package..."
    
    local package_dir="${PROJECT_ROOT}/deploy/package_${BUILD_ID}"
    mkdir -p "$package_dir"
    
    # Copy binary
    cp target/production/hft_stealth_system "$package_dir/"
    
    # Copy configs
    cp -r config "$package_dir/"
    
    # Copy scripts
    cp scripts/run_prod.sh "$package_dir/"
    chmod +x "$package_dir/run_prod.sh"
    
    # Copy systemd service
    cp deploy/systemd/hft-stealth.service "$package_dir/"
    
    # Create version file
    echo "VERSION=$VERSION" > "$package_dir/version.txt"
    echo "BUILD_ID=$BUILD_ID" >> "$package_dir/version.txt"
    echo "BUILD_TIME=$(date -Iseconds)" >> "$package_dir/version.txt"
    
    # Create tarball
    cd "$PROJECT_ROOT"
    tar -czf "hft-stealth-${BUILD_ID}.tar.gz" -C deploy "package_${BUILD_ID}"
    
    log "Package created: hft-stealth-${BUILD_ID}.tar.gz"
}

deploy_blue_green() {
    log "Performing blue-green deployment to $DEPLOY_HOST..."
    
    local current_color="blue"
    local new_color="green"
    
    # Determine current active color
    if ssh "$DEPLOY_USER@$DEPLOY_HOST" "test -L ${DEPLOY_PATH}/current && readlink ${DEPLOY_PATH}/current | grep -q green"; then
        current_color="green"
        new_color="blue"
    fi
    
    log "Current active: $current_color, deploying to: $new_color"
    
    # Upload package
    local remote_path="${DEPLOY_PATH}/releases/${BUILD_ID}"
    ssh "$DEPLOY_USER@$DEPLOY_HOST" "mkdir -p $remote_path"
    scp "hft-stealth-${BUILD_ID}.tar.gz" "$DEPLOY_USER@$DEPLOY_HOST:$remote_path/"
    
    # Extract on remote
    ssh "$DEPLOY_USER@$DEPLOY_HOST" "cd $remote_path && tar -xzf hft-stealth-${BUILD_ID}.tar.gz && rm hft-stealth-${BUILD_ID}.tar.gz"
    
    # Create symlink for new color
    ssh "$DEPLOY_USER@$DEPLOY_HOST" "ln -sfn $remote_path ${DEPLOY_PATH}/${new_color}"
    
    # Start new version
    ssh "$DEPLOY_USER@$DEPLOY_HOST" "cd ${DEPLOY_PATH}/${new_color} && ./run_prod.sh start"
    
    # Health check
    if ! health_check; then
        error "Health check failed for $new_color"
        if [[ "$ROLLBACK_ENABLED" == "true" ]]; then
            rollback "$current_color"
        fi
        exit 1
    fi
    
    # Switch traffic
    ssh "$DEPLOY_USER@$DEPLOY_HOST" "ln -sfn ${DEPLOY_PATH}/${new_color} ${DEPLOY_PATH}/current"
    
    # Stop old version
    ssh "$DEPLOY_USER@$DEPLOY_HOST" "cd ${DEPLOY_PATH}/${current_color} && ./run_prod.sh stop || true"
    
    # Backup old version
    ssh "$DEPLOY_USER@$DEPLOY_HOST" "mkdir -p ${BACKUP_PATH} && cp -r ${DEPLOY_PATH}/${current_color} ${BACKUP_PATH}/${BUILD_ID}_${current_color}"
    
    log "Blue-green deployment complete. Active: $new_color"
}

deploy_rolling() {
    log "Performing rolling deployment..."
    
    # For multi-instance deployments
    local instances=($(get_instances))
    
    for instance in "${instances[@]}"; do
        log "Deploying to $instance..."
        
        # Take instance out of load balancer
        remove_from_lb "$instance"
        
        # Deploy to instance
        ssh "$instance" "cd ${DEPLOY_PATH} && ./run_prod.sh stop || true"
        scp "hft-stealth-${BUILD_ID}.tar.gz" "$instance:${DEPLOY_PATH}/"
        ssh "$instance" "cd ${DEPLOY_PATH} && tar -xzf hft-stealth-${BUILD_ID}.tar.gz"
        ssh "$instance" "cd ${DEPLOY_PATH} && ./run_prod.sh start"
        
        # Health check
        if health_check "$instance"; then
            # Add back to load balancer
            add_to_lb "$instance"
        else
            error "Health check failed for $instance"
            rollback_instance "$instance"
        fi
        
        # Wait for stabilization
        sleep 5
    done
    
    log "Rolling deployment complete"
}

deploy_canary() {
    log "Performing canary deployment ($CANARY_PERCENT% traffic)..."
    
    # Deploy canary instance
    local canary_host="canary-${DEPLOY_HOST}"
    
    ssh "$DEPLOY_USER@$canary_host" "mkdir -p ${DEPLOY_PATH}/canary"
    scp "hft-stealth-${BUILD_ID}.tar.gz" "$DEPLOY_USER@$canary_host:${DEPLOY_PATH}/canary/"
    ssh "$DEPLOY_USER@$canary_host" "cd ${DEPLOY_PATH}/canary && tar -xzf hft-stealth-${BUILD_ID}.tar.gz"
    ssh "$DEPLOY_USER@$canary_host" "cd ${DEPLOY_PATH}/canary && ./run_prod.sh start"
    
    # Route $CANARY_PERCENT traffic to canary
    configure_traffic_split "$CANARY_PERCENT"
    
    # Monitor canary for observation period (5 minutes)
    log "Monitoring canary for 5 minutes..."
    sleep 300
    
    # Check canary health
    if health_check "$canary_host"; then
        log "Canary healthy, proceeding with full deployment"
        deploy_blue_green
    else
        error "Canary unhealthy, rolling back"
        configure_traffic_split 0
        ssh "$DEPLOY_USER@$canary_host" "cd ${DEPLOY_PATH}/canary && ./run_prod.sh stop"
        exit 1
    fi
}

health_check() {
    local host="${1:-$DEPLOY_HOST}"
    local retries=0
    
    while [[ $retries -lt $HEALTH_CHECK_RETRIES ]]; do
        if ssh "$DEPLOY_USER@$host" "curl -sf ${HEALTH_CHECK_URL} > /dev/null 2>&1"; then
            log "Health check passed for $host"
            return 0
        fi
        
        retries=$((retries + 1))
        log "Health check attempt $retries failed for $host"
        sleep 5
    done
    
    error "Health check failed for $host after $HEALTH_CHECK_RETRIES attempts"
    return 1
}

rollback() {
    local target_color="$1"
    
    log "Rolling back to $target_color..."
    
    ssh "$DEPLOY_USER@$DEPLOY_HOST" "ln -sfn ${DEPLOY_PATH}/${target_color} ${DEPLOY_PATH}/current"
    ssh "$DEPLOY_USER@$DEPLOY_HOST" "cd ${DEPLOY_PATH}/${target_color} && ./run_prod.sh start"
    
    if health_check; then
        log "Rollback successful"
    else
        error "Rollback failed!"
        exit 1
    fi
}

verify_deployment() {
    log "Verifying deployment..."
    
    # Check version
    local deployed_version=$(ssh "$DEPLOY_USER@$DEPLOY_HOST" "cat ${DEPLOY_PATH}/current/version.txt | grep VERSION | cut -d= -f2")
    
    if [[ "$deployed_version" != "$VERSION" ]]; then
        error "Version mismatch: expected $VERSION, got $deployed_version"
        return 1
    fi
    
    # Run smoke tests
    log "Running smoke tests..."
    
    # Test health endpoint
    if ! health_check; then
        error "Health check failed"
        return 1
    fi
    
    # Test metrics endpoint
    if ! ssh "$DEPLOY_USER@$DEPLOY_HOST" "curl -sf http://localhost:9090/metrics > /dev/null"; then
        error "Metrics endpoint not available"
        return 1
    fi
    
    log "Deployment verification passed"
    return 0
}

# ============================================================
# MAIN
# ============================================================

case "${1:-}" in
    build)
        build
        ;;
    package)
        build
        package
        ;;
    deploy)
        build
        package
        
        case "$STRATEGY" in
            blue-green)
                deploy_blue_green
                ;;
            rolling)
                deploy_rolling
                ;;
            canary)
                deploy_canary
                ;;
            *)
                error "Unknown strategy: $STRATEGY"
                exit 1
                ;;
        esac
        
        verify_deployment
        ;;
    rollback)
        rollback "$2"
        ;;
    verify)
        verify_deployment
        ;;
    *)
        echo "Usage: $0 {build|package|deploy|rollback <color>|verify}"
        echo ""
        echo "Environment variables:"
        echo "  DEPLOY_USER    - SSH user (default: hft)"
        echo "  DEPLOY_HOST    - Target host (default: prod-server-01)"
        echo "  STRATEGY       - blue-green, rolling, canary (default: blue-green)"
        echo "  CANARY_PERCENT - Traffic percentage for canary (default: 10)"
        exit 1
        ;;
esac

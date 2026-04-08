// ============================================================
// HFT STEALTH SYSTEM - MAIN ENTRY POINT
// ============================================================
// Production binary with CLI, signal handling, and process management
// ============================================================

#![cfg_attr(production, deny(warnings))]

use std::sync::Arc;
use std::process;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use tokio::signal;
use tracing::{info, error, warn, debug};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use hft_7zero::{
    HFTStealthSystem,
    SystemConfig,
    Component,
    HealthStatus,
    io::PacketCapture,
    ml::JAXModel,
    execution::StealthExecutor,
};
use std::time::Duration;

// ============================================================
// COMMAND LINE INTERFACE
// ============================================================

#[derive(Parser)]
#[command(name = "hft-stealth")]
#[command(author = "HFT Stealth Team")]
#[command(version = "1.0.0")]
#[command(about = "Production HFT Stealth Execution System", long_about = None)]
struct Cli {
    /// Configuration file path
    #[arg(short, long, default_value = "config/production.toml")]
    config: PathBuf,
    
    /// Run in development mode
    #[arg(short, long)]
    dev: bool,
    
    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
    
    /// Enable JSON log output
    #[arg(long)]
    json_logs: bool,
    
    /// Override latency budget (microseconds)
    #[arg(long)]
    latency_budget_us: Option<u64>,
    
    /// Subcommand
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Clone)]
enum Commands {
    /// Run the trading system
    Run {
        /// Dry run (no actual orders)
        #[arg(long)]
        dry_run: bool,
        
        /// Backtest mode
        #[arg(long)]
        backtest: bool,
        
        /// Backtest data file
        #[arg(long)]
        backtest_data: Option<PathBuf>,
    },
    
    /// Benchmark system performance
    Benchmark {
        /// Benchmark duration in seconds
        #[arg(short, long, default_value = "30")]
        duration: u64,
        
        /// Benchmark type
        #[arg(short, long, default_value = "latency")]
        bench_type: String,
    },
    
    /// Validate configuration
    Validate,
    
    /// Show system status
    Status,
    
    /// Generate performance report
    Report {
        /// Output format (json, text, html)
        #[arg(short, long, default_value = "text")]
        format: String,
        
        /// Output file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

// ============================================================
// MAIN ENTRY POINT
// ============================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    // Initialize logging
    init_logging(&cli).await?;
    
    // Handle subcommands
    if let Some(cmd) = cli.command.clone() {
        return handle_command(cmd, &cli).await;
    }
    
    // Default: run system
    run_trading_system(&cli, false, false, None).await
}

// ============================================================
// LOGGING INITIALIZATION
// ============================================================

async fn init_logging(cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    let env_filter = if cli.verbose {
        EnvFilter::new("hft_stealth_system=debug,info")
    } else if cli.dev {
        EnvFilter::new("hft_stealth_system=debug,warn")
    } else {
        EnvFilter::new("hft_stealth_system=info,error")
    };
    
    let subscriber = fmt::Subscriber::builder()
        .with_env_filter(env_filter)
        .with_thread_ids(true);

    if cli.json_logs {
        subscriber.json().init();
    } else {
        subscriber.pretty().init();
    }
    
    info!("Logging initialized (verbose={}, json={})", cli.verbose, cli.json_logs);
    Ok(())
}

// ============================================================
// COMMAND HANDLERS
// ============================================================

async fn handle_command(cmd: Commands, cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        Commands::Run { dry_run, backtest, backtest_data } => {
            run_trading_system(cli, dry_run, backtest, backtest_data).await
        }
        Commands::Benchmark { duration, bench_type } => {
            run_benchmark(duration, &bench_type).await
        }
        Commands::Validate => {
            validate_configuration(cli).await
        }
        Commands::Status => {
            show_status().await
        }
        Commands::Report { format, output } => {
            generate_report(&format, output).await
        }
    }
}

// ============================================================
// TRADING SYSTEM EXECUTION
// ============================================================

async fn run_trading_system(
    cli: &Cli,
    dry_run: bool,
    backtest: bool,
    backtest_data: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting HFT Stealth Trading System");
    info!("Mode: {} | Dry run: {} | Backtest: {}", 
          if cli.dev { "Development" } else { "Production" },
          dry_run,
          backtest);
    
    // Load configuration
    let mut config = SystemConfig::from_file(&cli.config)?;
    
    if let Some(budget_us) = cli.latency_budget_us {
        config.latency_budget_ns = budget_us * 1000;
        info!("Overriding latency budget to {}μs", budget_us);
    }
    
    if dry_run {
        config.dry_run = true;
        info!("Running in DRY RUN mode - no orders will be sent");
    }
    
    if backtest {
        config.backtest_mode = true;
        if let Some(data_file) = backtest_data {
            config.backtest_data_file = data_file;
        }
        info!("Running in BACKTEST mode");
    }
    
    // Create system instance
    let system = HFTStealthSystem::new(config)?;
    
    // Register components
    register_components(&system).await?;
    
    // Setup signal handling
    let mut shutdown_signals = setup_signal_handling();
    
    // Start system
    if let Err(e) = system.start().await {
        error!("Failed to start system: {}", e);
        return Err(e.into());
    }
    
    info!("System running. Press Ctrl+C to stop.");
    
    // Wait for shutdown signal
    tokio::select! {
        _ = shutdown_signals.recv() => {
            info!("Shutdown signal received");
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Ctrl+C received");
        }
    }
    
    // Graceful shutdown
    info!("Initiating graceful shutdown...");
    if let Err(e) = system.stop().await {
        error!("Error during shutdown: {}", e);
    }
    
    // Print final metrics
    print_final_metrics(&system).await;
    
    info!("System shutdown complete");
    Ok(())
}

// ============================================================
// COMPONENT REGISTRATION
// ============================================================

async fn register_components(system: &HFTStealthSystem) -> Result<(), Box<dyn std::error::Error>> {
    // Register packet capture component
    let packet_capture = PacketCapture::new()?;
    system.register_component(packet_capture);
    
    // Register JAX model
    let jax_model = JAXModel::new()?;
    system.register_component(jax_model);
    
    // Register stealth executor
    let stealth_executor = StealthExecutor::new();
    system.register_component(stealth_executor);
    
    info!("Registered {} components", 3);
    Ok(())
}

// ============================================================
// SIGNAL HANDLING
// ============================================================

fn setup_signal_handling() -> tokio::sync::mpsc::Receiver<()> {
    let (tx, rx) = tokio::sync::mpsc::channel(1);
    
    // Handle SIGTERM for graceful shutdown in production
    tokio::spawn(async move {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
                info!("Received SIGTERM");
                let _ = tx.send(()).await;
            }
            Err(e) => {
                error!("Failed to setup signal handler: {}", e);
            }
        }
    });
    
    rx
}

// ============================================================
// BENCHMARKING
// ============================================================

async fn run_benchmark(duration: u64, bench_type: &str) -> Result<(), Box<dyn std::error::Error>> {
    info!("Running benchmark: type={}, duration={}s", bench_type, duration);
    
    match bench_type {
        "latency" => benchmark_latency(duration).await,
        "throughput" => benchmark_throughput(duration).await,
        "risk" => benchmark_risk_computation(duration).await,
        "full" => benchmark_full_system(duration).await,
        _ => {
            error!("Unknown benchmark type: {}", bench_type);
            process::exit(1);
        }
    }
    
    Ok(())
}

async fn benchmark_latency(duration: u64) {
    use std::time::Instant;
    
    let mut latencies = Vec::with_capacity(1_000_000);
    let start = Instant::now();
    let end = start + Duration::from_secs(duration);
    
    while Instant::now() < end {
        let cycle_start = Instant::now();
        
        // Simulate one processing cycle
        std::thread::yield_now();
        
        let latency_ns = cycle_start.elapsed().as_nanos() as u64;
        latencies.push(latency_ns);
    }
    
    // Compute statistics
    latencies.sort();
    let p50 = latencies[latencies.len() / 2];
    let p99 = latencies[(latencies.len() * 99) / 100];
    let p999 = latencies[(latencies.len() * 999) / 1000];
    let max = *latencies.last().unwrap();
    let min = latencies[0];
    let avg = latencies.iter().sum::<u64>() / latencies.len() as u64;
    
    println!("\n=== Latency Benchmark Results ===");
    println!("Samples: {}", latencies.len());
    println!("Min:     {} ns ({} μs)", min, min / 1000);
    println!("P50:     {} ns ({} μs)", p50, p50 / 1000);
    println!("P99:     {} ns ({} μs)", p99, p99 / 1000);
    println!("P999:    {} ns ({} μs)", p999, p999 / 1000);
    println!("Max:     {} ns ({} μs)", max, max / 1000);
    println!("Avg:     {} ns ({} μs)", avg, avg / 1000);
}

async fn benchmark_throughput(duration: u64) {
    // Throughput benchmark implementation
    info!("Throughput benchmark not yet implemented");
}

async fn benchmark_risk_computation(duration: u64) {
    // Risk computation benchmark
    info!("Risk benchmark not yet implemented");
}

async fn benchmark_full_system(duration: u64) {
    // Full system benchmark
    info!("Full system benchmark not yet implemented");
}

// ============================================================
// CONFIGURATION VALIDATION
// ============================================================

async fn validate_configuration(cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    info!("Validating configuration: {}", cli.config.display());
    
    match SystemConfig::from_file(&cli.config) {
        Ok(config) => {
            info!("Configuration valid!");
            info!("  Latency budget: {} μs", config.latency_budget_ns / 1000);
            info!("  Max position: {} lots", config.max_position_lots);
            info!("  Risk threshold: {}", config.risk_threshold);
            info!("  Stealth enabled: {}", config.stealth_enabled);
            info!("  Instruments: {}", config.instruments.len());
            Ok(())
        }
        Err(e) => {
            error!("Configuration validation failed: {}", e);
            Err(e.into())
        }
    }
}

// ============================================================
// STATUS DISPLAY
// ============================================================

async fn show_status() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== HFT Stealth System Status ===");
    println!("System: Not running (use 'run' command to start)");
    println!("Version: {}", env!("CARGO_PKG_VERSION"));
    println!("Features: production");
    println!();
    println!("To start the system: cargo run -- run");
    println!("To benchmark: cargo run -- benchmark");
    println!("To validate config: cargo run -- validate");
    
    Ok(())
}

// ============================================================
// REPORT GENERATION
// ============================================================

async fn generate_report(format: &str, output: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    info!("Generating report in {} format", format);
    
    // In production, this would read from metrics database
    let report_data = serde_json::json!({
        "system": "HFT Stealth",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_seconds": 0,
        "metrics": {
            "total_trades": 0,
            "total_pnl": 0.0,
            "sharpe_ratio": 0.0,
            "win_rate": 0.0,
            "avg_latency_us": 0,
            "detection_probability": 0.0,
        }
    });
    
    match format {
        "json" => {
            let json_str = serde_json::to_string_pretty(&report_data)?;
            if let Some(path) = output {
                std::fs::write(path, json_str)?;
            } else {
                println!("{}", json_str);
            }
        }
        "html" => {
            let html = generate_html_report(&report_data);
            if let Some(path) = output {
                std::fs::write(path, html)?;
            } else {
                println!("{}", html);
            }
        }
        "text" => {
            print_text_report(&report_data);
        }
        _ => {
            error!("Unknown format: {}", format);
            process::exit(1);
        }
    }
    
    Ok(())
}

fn generate_html_report(data: &serde_json::Value) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head><title>HFT Stealth Report</title></head>
<body>
<h1>System Performance Report</h1>
<pre>{}</pre>
</body>
</html>"#,
        serde_json::to_string_pretty(data).unwrap()
    )
}

fn print_text_report(data: &serde_json::Value) {
    println!("\n=== HFT Stealth Performance Report ===\n");
    println!("System: {}", data["system"].as_str().unwrap());
    println!("Version: {}", data["version"].as_str().unwrap());
    println!("\nMetrics:");
    if let Some(metrics) = data["metrics"].as_object() {
        for (key, value) in metrics {
            println!("  {}: {}", key, value);
        }
    }
}

// ============================================================
// FINAL METRICS
// ============================================================

async fn print_final_metrics(system: &HFTStealthSystem) {
    let metrics = system.get_metrics();
    
    println!("\n=== Final System Metrics ===");
    println!("Total trades: {}", metrics.total_trades);
    println!("Total P&L: ${:.2}", metrics.total_pnl);
    println!("Sharpe ratio: {:.3}", metrics.sharpe_ratio);
    println!("P99 Latency: {} μs", metrics.latency_p99_ns / 1000);
    println!("Throughput: {:.0} ticks/sec", metrics.throughput_ticks_sec);
    println!("Detection probability: {:.6}%", metrics.detection_probability * 100.0);
    
    let health = system.health_check();
    println!("System health: {:?}", health);
}

// ============================================================
// GLOBAL ALLOCATOR (Performance)
// ============================================================

#[cfg(feature = "optimized-memory")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(not(feature = "optimized-memory"))]
#[global_allocator]
static GLOBAL: std::alloc::System = std::alloc::System;

// ============================================================
// PANIC HOOK
// ============================================================

#[cfg(production)]
fn setup_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        error!("Panic occurred: {}", panic_info);
        default_hook(panic_info);
        process::exit(1);
    }));
}

#[cfg(not(production))]
fn setup_panic_hook() {
    // Keep default panic behavior in development
}

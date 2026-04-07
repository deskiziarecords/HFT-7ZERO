// ============================================================
// SIGNAL PROCESSING MODULE
// ============================================================
// Harmonic trap detection with spectral analysis
// KL divergence for distribution comparison
// Mandra gate for regime change detection
// Real-time FFT and wavelet transforms
// ============================================================

pub mod harmonic_detector;
pub mod spectral;
pub mod kl_divergence;
pub mod mandra_gate;
pub mod wavelet;
pub mod filter_bank;

pub use harmonic_detector::{HarmonicTrapDetector, HarmonicConfig, TrapType};
pub use spectral::{SpectralAnalyzer, SpectralFeatures, PowerSpectrum, PhaseSpectrum};
pub use kl_divergence::{KLDivergence, DistributionComparator, DivergenceResult};
pub use mandra_gate::{MandraGate, MandraConfig, GateState, EnergyThreshold};
pub use wavelet::{WaveletTransform, WaveletType, DecompositionLevel};
pub use filter_bank::{FilterBank, FilterType, AdaptiveFilter};

use std::sync::Arc;
use parking_lot::RwLock;
use dashmap::DashMap;
use rustfft::FftPlanner;
use num_complex::Complex;

/// Signal processing configuration
#[derive(Debug, Clone)]
pub struct SignalConfig {
    pub fft_size: usize,
    pub window_type: WindowType,
    pub overlap_factor: f64,
    pub spectral_threshold: f64,
    pub kl_epsilon: f64,
    pub mandra_energy_threshold: f64,
    pub harmonic_phase_threshold: f64,
}

impl Default for SignalConfig {
    fn default() -> Self {
        Self {
            fft_size: 256,
            window_type: WindowType::Hanning,
            overlap_factor: 0.5,
            spectral_threshold: 0.1,
            kl_epsilon: 0.01,
            mandra_energy_threshold: 2.0,
            harmonic_phase_threshold: std::f64::consts::PI / 2.0,
        }
    }
}

/// Window types for spectral analysis
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowType {
    Rectangular,
    Hanning,
    Hamming,
    Blackman,
    BlackmanHarris,
    FlatTop,
}

impl WindowType {
    pub fn apply(&self, n: usize) -> Vec<f64> {
        match self {
            WindowType::Rectangular => vec![1.0; n],
            WindowType::Hanning => (0..n).map(|i| {
                0.5 * (1.0 - (2.0 * std::f64::consts::PI * i as f64 / (n - 1) as f64).cos())
            }).collect(),
            WindowType::Hamming => (0..n).map(|i| {
                0.54 - 0.46 * (2.0 * std::f64::consts::PI * i as f64 / (n - 1) as f64).cos()
            }).collect(),
            WindowType::Blackman => (0..n).map(|i| {
                let a0 = 0.42;
                let a1 = 0.5;
                let a2 = 0.08;
                let theta = 2.0 * std::f64::consts::PI * i as f64 / (n - 1) as f64;
                a0 - a1 * theta.cos() + a2 * (2.0 * theta).cos()
            }).collect(),
            WindowType::BlackmanHarris => (0..n).map(|i| {
                let a0 = 0.35875;
                let a1 = 0.48829;
                let a2 = 0.14128;
                let a3 = 0.01168;
                let theta = 2.0 * std::f64::consts::PI * i as f64 / (n - 1) as f64;
                a0 - a1 * theta.cos() + a2 * (2.0 * theta).cos() - a3 * (3.0 * theta).cos()
            }).collect(),
            WindowType::FlatTop => (0..n).map(|i| {
                let a0 = 1.0;
                let a1 = 1.93;
                let a2 = 1.29;
                let a3 = 0.388;
                let a4 = 0.032;
                let theta = 2.0 * std::f64::consts::PI * i as f64 / (n - 1) as f64;
                a0 - a1 * theta.cos() + a2 * (2.0 * theta).cos() - a3 * (3.0 * theta).cos() + a4 * (4.0 * theta).cos()
            }).collect(),
        }
    }
}

/// Signal quality metrics
#[derive(Debug, Clone, Default)]
pub struct SignalQuality {
    pub snr_db: f64,
    pub thd_percent: f64,
    pub phase_noise_db: f64,
    pub spectral_flatness: f64,
    pub kurtosis: f64,
    pub timestamp_ns: u64,
}

/// Global signal processor
pub static SIGNAL_PROCESSOR: once_cell::sync::Lazy<Arc<SignalProcessor>> = 
    once_cell::sync::Lazy::new(|| Arc::new(SignalProcessor::new()));

/// Main signal processor
pub struct SignalProcessor {
    config: RwLock<SignalConfig>,
    fft_planner: RwLock<FftPlanner<f64>>,
    cache: DashMap<u64, SpectralFeatures>,
}

impl SignalProcessor {
    pub fn new() -> Self {
        Self {
            config: RwLock::new(SignalConfig::default()),
            fft_planner: RwLock::new(FftPlanner::new()),
            cache: DashMap::with_capacity(1000),
        }
    }
    
    pub fn set_config(&self, config: SignalConfig) {
        *self.config.write() = config;
    }
    
    pub fn get_config(&self) -> SignalConfig {
        self.config.read().clone()
    }
    
    pub fn clear_cache(&self) {
        self.cache.clear();
    }
}

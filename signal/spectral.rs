// ============================================================
// SPECTRAL ANALYSIS
// ============================================================
// Power spectral density estimation
// Phase spectrum analysis
// Spectral features for ML
// Real-time spectral tracking
// ============================================================

use super::*;
use rustfft::{FftPlanner, NumOps};
use std::collections::VecDeque;

/// Power spectrum with frequency bins
#[derive(Debug, Clone)]
pub struct PowerSpectrum {
    pub frequencies: Vec<f64>,
    pub magnitudes: Vec<f64>,
    pub phases: Vec<f64>,
    pub sample_rate: f64,
    pub timestamp_ns: u64,
}

/// Phase spectrum for causality analysis
#[derive(Debug, Clone)]
pub struct PhaseSpectrum {
    pub frequencies: Vec<f64>,
    pub phase_differences: Vec<f64>,
    pub coherence: Vec<f64>,
    pub group_delay: Vec<f64>,
}

/// Spectral features for machine learning
#[derive(Debug, Clone, Default)]
pub struct SpectralFeatures {
    pub spectral_centroid: f64,
    pub spectral_spread: f64,
    pub spectral_skewness: f64,
    pub spectral_kurtosis: f64,
    pub spectral_rolloff: f64,
    pub spectral_flux: f64,
    pub dominant_frequency: f64,
    pub dominant_magnitude: f64,
    pub harmonic_ratio: f64,
    pub noise_floor: f64,
}

/// Real-time spectral analyzer
pub struct SpectralAnalyzer {
    config: SignalConfig,
    fft_planner: FftPlanner<f64>,
    window: Vec<f64>,
    history: VecDeque<PowerSpectrum>,
    prev_spectrum: Option<PowerSpectrum>,
}

impl SpectralAnalyzer {
    /// Create new spectral analyzer
    pub fn new(config: SignalConfig) -> Self {
        let window = config.window_type.apply(config.fft_size);
        
        Self {
            config,
            fft_planner: FftPlanner::new(),
            window,
            history: VecDeque::with_capacity(100),
            prev_spectrum: None,
        }
    }
    
    /// Compute power spectrum of signal
    pub fn compute_spectrum(&mut self, signal: &[f64], sample_rate: f64) -> PowerSpectrum {
        let n = signal.len().min(self.config.fft_size);
        let mut complex_signal: Vec<Complex<f64>> = (0..n)
            .map(|i| Complex::new(signal[i] * self.window[i], 0.0))
            .collect();
        
        // Zero-pad if necessary
        while complex_signal.len() < self.config.fft_size {
            complex_signal.push(Complex::new(0.0, 0.0));
        }
        
        let fft = self.fft_planner.plan_fft_forward(self.config.fft_size);
        fft.process(&mut complex_signal);
        
        let nyquist = sample_rate / 2.0;
        let bin_width = sample_rate / self.config.fft_size as f64;
        
        let mut frequencies = Vec::with_capacity(self.config.fft_size / 2);
        let mut magnitudes = Vec::with_capacity(self.config.fft_size / 2);
        let mut phases = Vec::with_capacity(self.config.fft_size / 2);
        
        for i in 1..self.config.fft_size / 2 {
            let freq = i as f64 * bin_width;
            if freq <= nyquist {
                frequencies.push(freq);
                let mag = complex_signal[i].norm() / n as f64;
                magnitudes.push(mag);
                phases.push(complex_signal[i].im.atan2(complex_signal[i].re));
            }
        }
        
        let spectrum = PowerSpectrum {
            frequencies,
            magnitudes,
            phases,
            sample_rate,
            timestamp_ns: crate::utils::time::get_hardware_timestamp(),
        };
        
        // Store history
        self.history.push_back(spectrum.clone());
        while self.history.len() > 100 {
            self.history.pop_front();
        }
        
        spectrum
    }
    
    /// Compute cross-spectrum between two signals
    pub fn cross_spectrum(&mut self, signal_a: &[f64], signal_b: &[f64], sample_rate: f64) -> PhaseSpectrum {
        let n = signal_a.len().min(signal_b.len()).min(self.config.fft_size);
        
        let mut complex_a: Vec<Complex<f64>> = (0..n)
            .map(|i| Complex::new(signal_a[i] * self.window[i], 0.0))
            .collect();
        
        let mut complex_b: Vec<Complex<f64>> = (0..n)
            .map(|i| Complex::new(signal_b[i] * self.window[i], 0.0))
            .collect();
        
        while complex_a.len() < self.config.fft_size {
            complex_a.push(Complex::new(0.0, 0.0));
            complex_b.push(Complex::new(0.0, 0.0));
        }
        
        let fft = self.fft_planner.plan_fft_forward(self.config.fft_size);
        fft.process(&mut complex_a);
        fft.process(&mut complex_b);
        
        let bin_width = sample_rate / self.config.fft_size as f64;
        let mut frequencies = Vec::with_capacity(self.config.fft_size / 2);
        let mut phase_diffs = Vec::with_capacity(self.config.fft_size / 2);
        let mut coherence = Vec::with_capacity(self.config.fft_size / 2);
        let mut group_delay = Vec::with_capacity(self.config.fft_size / 2);
        
        for i in 1..self.config.fft_size / 2 {
            let freq = i as f64 * bin_width;
            frequencies.push(freq);
            
            let phase_a = complex_a[i].im.atan2(complex_a[i].re);
            let phase_b = complex_b[i].im.atan2(complex_b[i].re);
            let phase_diff = (phase_a - phase_b).abs();
            phase_diffs.push(phase_diff);
            
            // Magnitude-squared coherence
            let cross_mag = (complex_a[i] * complex_b[i].conj()).norm();
            let mag_a = complex_a[i].norm();
            let mag_b = complex_b[i].norm();
            let coh = cross_mag / (mag_a * mag_b + 1e-8);
            coherence.push(coh);
            
            // Group delay: -dφ/dω
            if i > 1 {
                let prev_diff = phase_diffs[i - 2];
                let delta_omega = 2.0 * PI * bin_width;
                let delay = -(phase_diff - prev_diff) / delta_omega;
                group_delay.push(delay.max(0.0));
            } else {
                group_delay.push(0.0);
            }
        }
        
        PhaseSpectrum {
            frequencies,
            phase_differences: phase_diffs,
            coherence,
            group_delay,
        }
    }
    
    /// Extract spectral features for ML
    pub fn extract_features(&mut self, spectrum: &PowerSpectrum) -> SpectralFeatures {
        let mut features = SpectralFeatures::default();
        
        if spectrum.magnitudes.is_empty() {
            return features;
        }
        
        // Spectral centroid (weighted mean of frequencies)
        let total_mag: f64 = spectrum.magnitudes.iter().sum();
        if total_mag > 0.0 {
            features.spectral_centroid = spectrum.frequencies.iter()
                .zip(spectrum.magnitudes.iter())
                .map(|(&f, &m)| f * m)
                .sum::<f64>() / total_mag;
        }
        
        // Spectral spread (standard deviation around centroid)
        let variance = spectrum.frequencies.iter()
            .zip(spectrum.magnitudes.iter())
            .map(|(&f, &m)| (f - features.spectral_centroid).powi(2) * m)
            .sum::<f64>() / total_mag;
        features.spectral_spread = variance.sqrt();
        
        // Spectral skewness
        features.spectral_skewness = spectrum.frequencies.iter()
            .zip(spectrum.magnitudes.iter())
            .map(|(&f, &m)| (f - features.spectral_centroid).powi(3) * m)
            .sum::<f64>() / (total_mag * features.spectral_spread.powi(3) + 1e-8);
        
        // Spectral kurtosis
        features.spectral_kurtosis = spectrum.frequencies.iter()
            .zip(spectrum.magnitudes.iter())
            .map(|(&f, &m)| (f - features.spectral_centroid).powi(4) * m)
            .sum::<f64>() / (total_mag * features.spectral_spread.powi(4) + 1e-8);
        
        // Spectral rolloff (frequency below which 85% of energy lies)
        let mut cum_energy = 0.0;
        for (freq, mag) in spectrum.frequencies.iter().zip(spectrum.magnitudes.iter()) {
            cum_energy += mag;
            if cum_energy / total_mag >= 0.85 {
                features.spectral_rolloff = *freq;
                break;
            }
        }
        
        // Spectral flux (change between consecutive spectra)
        if let Some(prev) = &self.prev_spectrum {
            let flux: f64 = spectrum.magnitudes.iter()
                .zip(prev.magnitudes.iter())
                .map(|(c, p)| (c - p).powi(2))
                .sum();
            features.spectral_flux = flux.sqrt();
        }
        
        // Dominant frequency and magnitude
        let max_idx = spectrum.magnitudes.iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0);
        
        features.dominant_frequency = spectrum.frequencies[max_idx];
        features.dominant_magnitude = spectrum.magnitudes[max_idx];
        
        // Harmonic ratio (energy at harmonics vs total)
        let fundamental = features.dominant_frequency;
        let mut harmonic_energy = 0.0;
        for (freq, mag) in spectrum.frequencies.iter().zip(spectrum.magnitudes.iter()) {
            for harmonic in 2..=5 {
                if (freq - fundamental * harmonic as f64).abs() < fundamental * 0.1 {
                    harmonic_energy += mag;
                    break;
                }
            }
        }
        features.harmonic_ratio = harmonic_energy / (total_mag + 1e-8);
        
        // Noise floor (minimum magnitude in high frequencies)
        let high_freq_start = spectrum.frequencies.len() * 3 / 4;
        features.noise_floor = spectrum.magnitudes[high_freq_start..]
            .iter()
            .fold(f64::INFINITY, |a, &b| a.min(b));
        
        self.prev_spectrum = Some(spectrum.clone());
        features
    }
    
    /// Compute spectral entropy (measure of disorder)
    pub fn spectral_entropy(&self, spectrum: &PowerSpectrum) -> f64 {
        let total_mag: f64 = spectrum.magnitudes.iter().sum();
        if total_mag < 1e-8 {
            return 0.0;
        }
        
        let normalized: Vec<f64> = spectrum.magnitudes.iter()
            .map(|&m| m / total_mag)
            .collect();
        
        -normalized.iter()
            .map(|&p| if p > 0.0 { p * p.ln() } else { 0.0 })
            .sum::<f64>()
    }
    
    /// Detect spectral anomalies
    pub fn detect_anomaly(&self, spectrum: &PowerSpectrum, threshold_sigma: f64) -> bool {
        if self.history.len() < 10 {
            return false;
        }
        
        // Compute mean and std of recent spectral centroids
        let centroids: Vec<f64> = self.history.iter()
            .map(|s| {
                let total: f64 = s.magnitudes.iter().sum();
                if total > 0.0 {
                    s.frequencies.iter()
                        .zip(s.magnitudes.iter())
                        .map(|(&f, &m)| f * m)
                        .sum::<f64>() / total
                } else {
                    0.0
                }
            })
            .collect();
        
        let mean = centroids.iter().sum::<f64>() / centroids.len() as f64;
        let variance = centroids.iter()
            .map(|&c| (c - mean).powi(2))
            .sum::<f64>() / centroids.len() as f64;
        let std_dev = variance.sqrt();
        
        // Compute current centroid
        let total_mag: f64 = spectrum.magnitudes.iter().sum();
        let current_centroid = if total_mag > 0.0 {
            spectrum.frequencies.iter()
                .zip(spectrum.magnitudes.iter())
                .map(|(&f, &m)| f * m)
                .sum::<f64>() / total_mag
        } else {
            0.0
        };
        
        (current_centroid - mean).abs() > threshold_sigma * std_dev
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_spectral_analysis() {
        let config = SignalConfig::default();
        let mut analyzer = SpectralAnalyzer::new(config);
        
        // Generate a sine wave
        let sample_rate = 1000.0;
        let signal: Vec<f64> = (0..512).map(|i| (2.0 * PI * 50.0 * i as f64 / sample_rate).sin()).collect();
        
        let spectrum = analyzer.compute_spectrum(&signal, sample_rate);
        let features = analyzer.extract_features(&spectrum);
        
        assert!(!spectrum.magnitudes.is_empty());
        assert!((features.dominant_frequency - 50.0).abs() < 5.0);
    }
}

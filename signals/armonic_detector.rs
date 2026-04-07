// ============================================================
// HARMONIC TRAP DETECTOR (Spectral Inversion)
// ============================================================
// Detects phase inversion between predicted and actual signals
// ∠(f̂_pred/f̂_act) > π/2 ⇒ Harmonic Trap
// Real-time FFT with windowing
// ============================================================

use super::*;
use rustfft::{FftPlanner, NumOps};
use std::f64::consts::PI;

/// Harmonic trap type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrapType {
    NoTrap,
    PhaseInversion,      // Phase shift > 90°
    FrequencyDoubling,   // 2x frequency component
    SubHarmonic,         // 0.5x frequency component
    BroadbandNoise,      // Flat spectrum
    SpectralFold,        // Aliasing pattern
}

/// Harmonic detection configuration
#[derive(Debug, Clone)]
pub struct HarmonicConfig {
    pub phase_threshold: f64,      // π/2 default
    pub magnitude_threshold: f64,
    pub min_frequency_hz: f64,
    pub max_frequency_hz: f64,
    pub window_size: usize,
    pub overlap: usize,
    pub use_hilbert: bool,
}

impl Default for HarmonicConfig {
    fn default() -> Self {
        Self {
            phase_threshold: PI / 2.0,
            magnitude_threshold: 0.1,
            min_frequency_hz: 0.5,
            max_frequency_hz: 50.0,
            window_size: 256,
            overlap: 128,
            use_hilbert: false,
        }
    }
}

/// Harmonic trap detector
pub struct HarmonicTrapDetector {
    config: HarmonicConfig,
    fft_planner: FftPlanner<f64>,
    window: Vec<f64>,
    prev_phase: Vec<f64>,
    prev_magnitude: Vec<f64>,
}

impl HarmonicTrapDetector {
    /// Create new harmonic trap detector
    pub fn new(window_size: usize) -> Self {
        let config = HarmonicConfig {
            window_size,
            ..Default::default()
        };
        let window = WindowType::Hanning.apply(window_size);
        
        Self {
            config,
            fft_planner: FftPlanner::new(),
            window,
            prev_phase: Vec::new(),
            prev_magnitude: Vec::new(),
        }
    }
    
    /// Detect harmonic trap by comparing predicted and actual signals
    /// Returns true if ∠(f̂_pred/f̂_act) > π/2
    pub fn detect_trap(&mut self, predicted: &[f64], actual: &[f64]) -> bool {
        let n = predicted.len().min(actual.len()).min(self.config.window_size);
        if n < self.config.window_size / 2 {
            return false;
        }
        
        // Apply window and compute FFT for both signals
        let mut pred_complex: Vec<Complex<f64>> = (0..n)
            .map(|i| Complex::new(predicted[i] * self.window[i], 0.0))
            .collect();
        
        let mut act_complex: Vec<Complex<f64>> = (0..n)
            .map(|i| Complex::new(actual[i] * self.window[i], 0.0))
            .collect();
        
        let fft = self.fft_planner.plan_fft_forward(n);
        fft.process(&mut pred_complex);
        fft.process(&mut act_complex);
        
        // Compute phase difference
        let mut phase_diffs = Vec::with_capacity(n / 2);
        let mut magnitude_ratios = Vec::with_capacity(n / 2);
        
        for i in 1..n / 2 {
            let pred_phase = pred_complex[i].im.atan2(pred_complex[i].re);
            let act_phase = act_complex[i].im.atan2(act_complex[i].re);
            let phase_diff = (pred_phase - act_phase).abs();
            
            let pred_mag = pred_complex[i].norm();
            let act_mag = act_complex[i].norm();
            let mag_ratio = if act_mag > 0.0 { pred_mag / act_mag } else { 0.0 };
            
            phase_diffs.push(phase_diff);
            magnitude_ratios.push(mag_ratio);
        }
        
        // Check if any significant frequency component exceeds phase threshold
        let max_phase_diff = phase_diffs.iter().fold(0.0, |a, &b| a.max(b));
        let avg_phase_diff = phase_diffs.iter().sum::<f64>() / phase_diffs.len() as f64;
        
        // Store for trend analysis
        self.prev_phase = phase_diffs;
        self.prev_magnitude = magnitude_ratios;
        
        // Trap condition: phase inversion > π/2
        max_phase_diff > self.config.phase_threshold || avg_phase_diff > self.config.phase_threshold * 0.7
    }
    
    /// Detect with detailed trap type classification
    pub fn detect_with_type(&mut self, predicted: &[f64], actual: &[f64]) -> (bool, TrapType) {
        let n = predicted.len().min(actual.len()).min(self.config.window_size);
        if n < self.config.window_size / 2 {
            return (false, TrapType::NoTrap);
        }
        
        let mut pred_complex: Vec<Complex<f64>> = (0..n)
            .map(|i| Complex::new(predicted[i] * self.window[i], 0.0))
            .collect();
        
        let mut act_complex: Vec<Complex<f64>> = (0..n)
            .map(|i| Complex::new(actual[i] * self.window[i], 0.0))
            .collect();
        
        let fft = self.fft_planner.plan_fft_forward(n);
        fft.process(&mut pred_complex);
        fft.process(&mut act_complex);
        
        let mut trap_type = TrapType::NoTrap;
        let mut phase_exceeded = false;
        let mut double_freq_detected = false;
        let mut half_freq_detected = false;
        
        for i in 1..n / 4 {
            let pred_phase = pred_complex[i].im.atan2(pred_complex[i].re);
            let act_phase = act_complex[i].im.atan2(act_complex[i].re);
            let phase_diff = (pred_phase - act_phase).abs();
            
            if phase_diff > self.config.phase_threshold {
                phase_exceeded = true;
            }
            
            // Check for frequency doubling (harmonic of fundamental)
            if i * 2 < n / 2 {
                let pred_mag_2x = pred_complex[i * 2].norm();
                let pred_mag_1x = pred_complex[i].norm();
                if pred_mag_2x > pred_mag_1x * 0.5 {
                    double_freq_detected = true;
                }
            }
            
            // Check for sub-harmonic
            if i % 2 == 0 && i > 0 {
                let pred_mag_half = pred_complex[i / 2].norm();
                let pred_mag_curr = pred_complex[i].norm();
                if pred_mag_half > pred_mag_curr * 0.8 {
                    half_freq_detected = true;
                }
            }
        }
        
        // Classify trap type
        if phase_exceeded {
            if double_freq_detected {
                trap_type = TrapType::FrequencyDoubling;
            } else if half_freq_detected {
                trap_type = TrapType::SubHarmonic;
            } else {
                trap_type = TrapType::PhaseInversion;
            }
        } else if self.check_broadband_noise(&pred_complex) {
            trap_type = TrapType::BroadbandNoise;
        } else if self.check_spectral_fold(&pred_complex, &act_complex) {
            trap_type = TrapType::SpectralFold;
        }
        
        (trap_type != TrapType::NoTrap, trap_type)
    }
    
    /// Check for broadband noise (flat spectrum)
    fn check_broadband_noise(&self, spectrum: &[Complex<f64>]) -> bool {
        let n = spectrum.len();
        if n < 10 {
            return false;
        }
        
        let magnitudes: Vec<f64> = (1..n / 2).map(|i| spectrum[i].norm()).collect();
        let mean = magnitudes.iter().sum::<f64>() / magnitudes.len() as f64;
        let variance = magnitudes.iter().map(|&m| (m - mean).powi(2)).sum::<f64>() / magnitudes.len() as f64;
        let std_dev = variance.sqrt();
        
        // High variance means non-flat spectrum
        std_dev / (mean + 1e-8) < 0.3
    }
    
    /// Check for spectral folding (aliasing pattern)
    fn check_spectral_fold(&self, pred: &[Complex<f64>], act: &[Complex<f64>]) -> bool {
        let n = pred.len().min(act.len());
        let mut fold_count = 0;
        
        for i in 1..n / 4 {
            let pred_mag = pred[i].norm();
            let act_mag = act[i].norm();
            let sym_idx = n / 2 - i;
            
            if sym_idx < n {
                let pred_sym = pred[sym_idx].norm();
                let act_sym = act[sym_idx].norm();
                
                if pred_mag > act_mag * 2.0 && pred_sym > act_sym * 2.0 {
                    fold_count += 1;
                }
            }
        }
        
        fold_count > n / 16
    }
    
    /// Real-time harmonic monitoring with sliding window
    pub fn monitor_stream(&mut self, stream: &mut dyn Iterator<Item = f64>, sample_rate: f64) -> Vec<bool> {
        let mut detections = Vec::new();
        let mut buffer = Vec::with_capacity(self.config.window_size);
        
        for sample in stream {
            buffer.push(sample);
            
            if buffer.len() >= self.config.window_size {
                let window_end = buffer.len();
                let window_start = window_end - self.config.window_size;
                let window: Vec<f64> = buffer[window_start..window_end].to_vec();
                
                // Need both predicted and actual - simplified for streaming
                let is_trap = self.detect_trap(&window, &window);
                detections.push(is_trap);
                
                // Slide window
                let step = self.config.window_size - self.config.overlap;
                if step > 0 {
                    buffer.drain(0..step);
                }
            }
        }
        
        detections
    }
    
    /// Get phase difference spectrum for analysis
    pub fn phase_spectrum(&mut self, predicted: &[f64], actual: &[f64]) -> Vec<f64> {
        let n = predicted.len().min(actual.len()).min(self.config.window_size);
        if n < 10 {
            return Vec::new();
        }
        
        let mut pred_complex: Vec<Complex<f64>> = (0..n)
            .map(|i| Complex::new(predicted[i] * self.window[i], 0.0))
            .collect();
        
        let mut act_complex: Vec<Complex<f64>> = (0..n)
            .map(|i| Complex::new(actual[i] * self.window[i], 0.0))
            .collect();
        
        let fft = self.fft_planner.plan_fft_forward(n);
        fft.process(&mut pred_complex);
        fft.process(&mut act_complex);
        
        (1..n / 2)
            .map(|i| {
                let pred_phase = pred_complex[i].im.atan2(pred_complex[i].re);
                let act_phase = act_complex[i].im.atan2(act_complex[i].re);
                (pred_phase - act_phase).abs()
            })
            .collect()
    }
    
    /// Get magnitude ratio spectrum
    pub fn magnitude_ratio_spectrum(&mut self, predicted: &[f64], actual: &[f64]) -> Vec<f64> {
        let n = predicted.len().min(actual.len()).min(self.config.window_size);
        if n < 10 {
            return Vec::new();
        }
        
        let mut pred_complex: Vec<Complex<f64>> = (0..n)
            .map(|i| Complex::new(predicted[i] * self.window[i], 0.0))
            .collect();
        
        let mut act_complex: Vec<Complex<f64>> = (0..n)
            .map(|i| Complex::new(actual[i] * self.window[i], 0.0))
            .collect();
        
        let fft = self.fft_planner.plan_fft_forward(n);
        fft.process(&mut pred_complex);
        fft.process(&mut act_complex);
        
        (1..n / 2)
            .map(|i| {
                let pred_mag = pred_complex[i].norm();
                let act_mag = act_complex[i].norm();
                if act_mag > 0.0 { pred_mag / act_mag } else { 0.0 }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_harmonic_detection() {
        let mut detector = HarmonicTrapDetector::new(256);
        
        // Generate in-phase signals (no trap)
        let in_phase_pred: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1).sin()).collect();
        let in_phase_act: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1).sin()).collect();
        
        assert!(!detector.detect_trap(&in_phase_pred, &in_phase_act));
        
        // Generate out-of-phase signals (trap)
        let out_phase_pred: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1).sin()).collect();
        let out_phase_act: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1 + PI).sin()).collect();
        
        assert!(detector.detect_trap(&out_phase_pred, &out_phase_act));
    }
}

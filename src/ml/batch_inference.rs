// ============================================================
// BATCH INFERENCE ENGINE
// ============================================================
// Dynamic batching for optimal throughput
// Latency-aware batch assembly
// Priority-based scheduling
// ============================================================

use super::*;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use crossbeam_channel::{bounded, Sender, Receiver, Select, tick};
use parking_lot::Mutex;

/// Batch inference configuration
#[derive(Debug, Clone)]
pub struct BatchConfig {
    pub max_batch_size: usize,
    pub max_batch_delay_ns: u64,
    pub min_batch_size: usize,
    pub num_worker_threads: usize,
    pub enable_priority: bool,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 32,
            max_batch_delay_ns: 100_000, // 100 microseconds
            min_batch_size: 1,
            num_worker_threads: 2,
            enable_priority: true,
        }
    }
}

/// Inference request with priority
#[derive(Debug)]
pub struct InferenceRequest {
    pub id: u64,
    pub tensor: Tensor,
    pub priority: u8,  // 0-255, higher = more important
    pub deadline_ns: u64,
    pub response_tx: Sender<Result<ModelOutput, String>>,
    pub timestamp_ns: u64,
}

/// Inference batch
#[derive(Debug)]
pub struct InferenceBatch {
    pub id: u64,
    pub requests: Vec<InferenceRequest>,
    pub created_at_ns: u64,
    pub batch_size: usize,
}

/// Batch inference engine
pub struct BatchInferenceEngine {
    model: Arc<JAXModel>,
    config: BatchConfig,
    request_queue: Arc<Mutex<VecDeque<InferenceRequest>>>,
    pending_batches: Arc<Mutex<VecDeque<InferenceBatch>>>,
    running: AtomicBool,
    next_batch_id: AtomicU64,
    next_request_id: AtomicU64,
    stats: Arc<Mutex<BatchStats>>,
    workers: Vec<std::thread::JoinHandle<()>>,
}

/// Batch statistics
#[derive(Debug, Default, Clone)]
pub struct BatchStats {
    pub batches_processed: u64,
    pub requests_processed: u64,
    pub avg_batch_size: f64,
    pub avg_batch_wait_ns: u64,
    pub avg_inference_time_ns: u64,
    pub batches_dropped: u64,
    pub priority_boosts: u64,
}

impl BatchInferenceEngine {
    /// Create new batch inference engine
    pub fn new(model: Arc<JAXModel>, config: BatchConfig) -> Self {
        Self {
            model,
            config,
            request_queue: Arc::new(Mutex::new(VecDeque::with_capacity(1024))),
            pending_batches: Arc::new(Mutex::new(VecDeque::with_capacity(64))),
            running: AtomicBool::new(false),
            next_batch_id: AtomicU64::new(0),
            next_request_id: AtomicU64::new(0),
            stats: Arc::new(Mutex::new(BatchStats::default())),
            workers: Vec::new(),
        }
    }
    
    /// Start batch inference engine
    pub fn start(&mut self) {
        self.running.store(true, Ordering::Release);
        
        // Start batch assembler thread
        let assembler_config = self.config.clone();
        let assembler_request_queue = self.request_queue.clone();
        let assembler_pending_batches = self.pending_batches.clone();
        let assembler_stats = self.stats.clone();
        let assembler_running = self.running.clone();
        
        self.workers.push(std::thread::spawn(move || {
            Self::batch_assembler_loop(
                assembler_config,
                assembler_request_queue,
                assembler_pending_batches,
                assembler_stats,
                assembler_running,
            );
        }));
        
        // Start worker threads
        for worker_id in 0..self.config.num_worker_threads {
            let model = self.model.clone();
            let pending_batches = self.pending_batches.clone();
            let stats = self.stats.clone();
            let running = self.running.clone();
            
            self.workers.push(std::thread::spawn(move || {
                Self::worker_loop(worker_id, model, pending_batches, stats, running);
            }));
        }
    }
    
    /// Stop batch inference engine
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Release);
        
        for handle in self.workers.drain(..) {
            let _ = handle.join();
        }
    }
    
    /// Submit inference request
    pub fn submit(&self, tensor: Tensor, priority: u8) -> Receiver<Result<ModelOutput, String>> {
        let (tx, rx) = bounded(1);
        
        let request = InferenceRequest {
            id: self.next_request_id.fetch_add(1, Ordering::Relaxed),
            tensor,
            priority,
            deadline_ns: 0, // No deadline
            response_tx: tx,
            timestamp_ns: crate::utils::time::get_hardware_timestamp(),
        };
        
        self.request_queue.lock().push_back(request);
        rx
    }
    
    /// Submit with deadline
    pub fn submit_with_deadline(
        &self,
        tensor: Tensor,
        priority: u8,
        deadline_ns: u64,
    ) -> Receiver<Result<ModelOutput, String>> {
        let (tx, rx) = bounded(1);
        
        let request = InferenceRequest {
            id: self.next_request_id.fetch_add(1, Ordering::Relaxed),
            tensor,
            priority,
            deadline_ns,
            response_tx: tx,
            timestamp_ns: crate::utils::time::get_hardware_timestamp(),
        };
        
        self.request_queue.lock().push_back(request);
        rx
    }
    
    /// Batch assembler loop
    fn batch_assembler_loop(
        config: BatchConfig,
        request_queue: Arc<Mutex<VecDeque<InferenceRequest>>>,
        pending_batches: Arc<Mutex<VecDeque<InferenceBatch>>>,
        stats: Arc<Mutex<BatchStats>>,
        running: AtomicBool,
    ) {
        let mut current_batch = Vec::with_capacity(config.max_batch_size);
        let mut batch_start_ns = 0u64;
        
        while running.load(Ordering::Acquire) {
            // Try to collect requests
            let mut queue = request_queue.lock();
            
            if current_batch.is_empty() {
                // Start new batch
                if let Some(request) = queue.pop_front() {
                    current_batch.push(request);
                    batch_start_ns = crate::utils::time::get_hardware_timestamp();
                }
            } else {
                // Collect more requests up to max batch size
                while current_batch.len() < config.max_batch_size {
                    if let Some(request) = queue.pop_front() {
                        // Check priority: if high priority, add immediately
                        if config.enable_priority && request.priority > 200 {
                            current_batch.push(request);
                            let mut stats_guard = stats.lock();
                            stats_guard.priority_boosts += 1;
                        } else {
                            // Check if we have room
                            if current_batch.len() < config.max_batch_size {
                                current_batch.push(request);
                            } else {
                                queue.push_front(request);
                                break;
                            }
                        }
                    } else {
                        break;
                    }
                }
            }
            
            drop(queue);
            
            // Check if batch is ready
            let now_ns = crate::utils::time::get_hardware_timestamp();
            let batch_age_ns = now_ns - batch_start_ns;
            
            let should_flush = !current_batch.is_empty() && (
                current_batch.len() >= config.max_batch_size ||
                (batch_age_ns >= config.max_batch_delay_ns && current_batch.len() >= config.min_batch_size)
            );
            
            if should_flush {
                let batch_id = crate::utils::time::get_hardware_timestamp();
                let batch = InferenceBatch {
                    id: batch_id,
                    requests: std::mem::take(&mut current_batch),
                    created_at_ns: batch_start_ns,
                    batch_size: current_batch.len(),
                };
                
                pending_batches.lock().push_back(batch);
                current_batch = Vec::with_capacity(config.max_batch_size);
                
                let mut stats_guard = stats.lock();
                stats_guard.batches_processed += 1;
            }
            
            // Small sleep to prevent busy loop
            std::thread::sleep(Duration::from_micros(10));
        }
    }
    
    /// Worker thread loop
    fn worker_loop(
        worker_id: usize,
        model: Arc<JAXModel>,
        pending_batches: Arc<Mutex<VecDeque<InferenceBatch>>>,
        stats: Arc<Mutex<BatchStats>>,
        running: AtomicBool,
    ) {
        while running.load(Ordering::Acquire) {
            // Try to get a batch
            let batch = {
                let mut batches = pending_batches.lock();
                batches.pop_front()
            };
            
            if let Some(batch) = batch {
                let start_time = Instant::now();
                
                // Extract tensors
                let tensors: Vec<Tensor> = batch.requests
                    .iter()
                    .map(|r| r.tensor.clone())
                    .collect();
                
                // Run batch inference
                match model.predict_batch(&tensors) {
                    Ok(outputs) => {
                        // Send responses
                        for (request, output) in batch.requests.iter().zip(outputs.iter()) {
                            let _ = request.response_tx.send(Ok(output.clone()));
                        }
                        
                        let inference_time_ns = start_time.elapsed().as_nanos() as u64;
                        
                        let mut stats_guard = stats.lock();
                        stats_guard.requests_processed += batch.requests.len();
                        stats_guard.avg_batch_size = stats_guard.avg_batch_size * 0.99 +
                            (batch.requests.len() as f64) * 0.01;
                        stats_guard.avg_inference_time_ns = stats_guard.avg_inference_time_ns * 0.99 +
                            inference_time_ns * 0.01;
                        
                        let wait_time_ns = start_time.elapsed().as_nanos() as u64 -
                            (batch.created_at_ns as u128) as u64;
                        stats_guard.avg_batch_wait_ns = stats_guard.avg_batch_wait_ns * 0.99 +
                            wait_time_ns * 0.01;
                    }
                    Err(e) => {
                        // Send error responses
                        for request in batch.requests {
                            let _ = request.response_tx.send(Err(e.clone()));
                        }
                        
                        let mut stats_guard = stats.lock();
                        stats_guard.batches_dropped += 1;
                    }
                }
            } else {
                // No batches, yield
                std::thread::yield_now();
            }
        }
    }
    
    /// Get statistics
    pub fn stats(&self) -> BatchStats {
        self.stats.lock().clone()
    }
    
    /// Get queue sizes
    pub fn queue_sizes(&self) -> (usize, usize) {
        let request_queue = self.request_queue.lock().len();
        let pending_batches = self.pending_batches.lock().len();
        (request_queue, pending_batches)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_batch_engine() {
        // Requires model to test
    }
}

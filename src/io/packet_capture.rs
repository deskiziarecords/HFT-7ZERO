// ============================================================
// PACKET CAPTURE
// ============================================================

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::io::ring_buffer::MPSCRingBuffer;

pub struct PacketCapture {
    pub running: Arc<AtomicBool>,
    pub buffer: Arc<MPSCRingBuffer>,
}

impl PacketCapture {
    pub fn new() -> std::io::Result<Self> {
        Ok(Self {
            running: Arc::new(AtomicBool::new(false)),
            buffer: Arc::new(MPSCRingBuffer::new(64 * 1024 * 1024)),
        })
    }
}

impl crate::Component for PacketCapture {
    fn name(&self) -> &'static str { "PacketCapture" }
    fn start(&self) -> Result<(), crate::SystemError> {
        self.running.store(true, Ordering::SeqCst);
        Ok(())
    }
    fn stop(&self) -> Result<(), crate::SystemError> {
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }
    fn health_check(&self) -> crate::HealthStatus {
        crate::HealthStatus::Healthy
    }
}

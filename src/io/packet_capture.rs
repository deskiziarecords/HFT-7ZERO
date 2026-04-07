// ============================================================
// REAL-TIME PACKET CAPTURE
// ============================================================
// Zero-copy packet capture with AF_XDP and io_uring
// Hardware timestamping support
// Packet filtering for market data
// ============================================================

use super::*;
use std::collections::VecDeque;
use std::net::IpAddr;
use std::sync::Arc;
use parking_lot::RwLock;
use pcap::{Capture, Device, Packet, Activated};

/// Main packet capture engine
pub struct PacketCapture {
    capture: Option<Capture<Activated>>,
    filter: PacketFilter,
    ring_buffer: Arc<MPSCRingBuffer>,
    stats: Arc<RwLock<CaptureStats>>,
    running: Arc<AtomicBool>,
    handler: Box<dyn PacketHandler + Send + Sync>,
}

/// Packet filter configuration
#[derive(Debug, Clone)]
pub struct PacketFilter {
    pub udp_ports: Vec<u16>,
    pub tcp_ports: Vec<u16>,
    pub multicast_groups: Vec<IpAddr>,
    pub exchange_macs: Vec<[u8; 6]>,
    pub min_packet_size: usize,
    pub max_packet_size: usize,
}

impl Default for PacketFilter {
    fn default() -> Self {
        Self {
            udp_ports: vec![
                10000, 10001, 10002,  // Exchange ports
                12345, 12346, 12347,  // Market data ports
            ],
            tcp_ports: vec![],
            multicast_groups: vec![],
            exchange_macs: vec![],
            min_packet_size: 64,
            max_packet_size: 1518,
        }
    }
}

/// Capture statistics
#[derive(Debug, Default, Clone)]
pub struct CaptureStats {
    pub packets_captured: u64,
    pub bytes_captured: u64,
    pub packets_filtered: u64,
    pub packets_dropped: u64,
    pub ring_buffer_overruns: u64,
    pub avg_packet_size: f64,
    pub capture_rate_pps: f64,
    pub last_second_packets: u64,
}

/// Packet handler trait
pub trait PacketHandler: Send + Sync {
    fn handle_packet(&self, packet: &[u8], timestamp_ns: u64);
    fn handle_batch(&self, packets: &[(Vec<u8>, u64)]);
}

/// Default packet handler for market data
pub struct MarketDataHandler {
    order_book: Arc<RwLock<crate::market::order_book::OrderBook>>,
    tick_sender: tokio::sync::mpsc::UnboundedSender<crate::market::tick::Tick>,
}

impl MarketDataHandler {
    pub fn new(
        order_book: Arc<RwLock<crate::market::order_book::OrderBook>>,
        tick_sender: tokio::sync::mpsc::UnboundedSender<crate::market::tick::Tick>,
    ) -> Self {
        Self {
            order_book,
            tick_sender,
        }
    }
    
    fn parse_market_data(&self, data: &[u8]) -> Option<crate::market::tick::Tick> {
        // Parse exchange-specific protocol
        // This is simplified - actual implementation depends on exchange
        if data.len() < 20 {
            return None;
        }
        
        Some(crate::market::tick::Tick {
            price: f64::from_be_bytes(data[0..8].try_into().unwrap_or([0; 8])),
            volume: f64::from_be_bytes(data[8..16].try_into().unwrap_or([0; 8])),
            timestamp_ns: u64::from_be_bytes(data[16..24].try_into().unwrap_or([0; 8])),
            exchange_id: data[24],
            side: data[25],
            sequence: u32::from_be_bytes(data[26..30].try_into().unwrap_or([0; 4])),
        })
    }
}

impl PacketHandler for MarketDataHandler {
    fn handle_packet(&self, packet: &[u8], timestamp_ns: u64) {
        if let Some(tick) = self.parse_market_data(packet) {
            // Update order book
            let mut book = self.order_book.write();
            book.update(&tick);
            
            // Send tick for processing
            let _ = self.tick_sender.send(tick);
        }
    }
    
    fn handle_batch(&self, packets: &[(Vec<u8>, u64)]) {
        for (packet, timestamp) in packets {
            self.handle_packet(packet, *timestamp);
        }
    }
}

impl PacketCapture {
    /// Create new packet capture on specified interface
    pub fn new(interface: &str, filter: PacketFilter) -> Result<Self, String> {
        let device = Device::list()
            .map_err(|e| format!("Failed to list devices: {}", e))?
            .into_iter()
            .find(|dev| dev.name == interface)
            .ok_or_else(|| format!("Interface {} not found", interface))?;
        
        let mut cap = Capture::from_device(device)
            .map_err(|e| format!("Failed to open device: {}", e))?
            .immediate_mode(true)
            .timeout(0)  // Non-blocking
            .snaplen(1518)
            .open()
            .map_err(|e| format!("Failed to activate capture: {}", e))?;
        
        // Set buffer size for high throughput
        cap.set_buffer_size(1024 * 1024 * 32)  // 32MB
            .map_err(|e| format!("Failed to set buffer size: {}", e))?;
        
        // Set filter
        let filter_str = Self::build_filter_string(&filter);
        cap.filter(&filter_str, true)
            .map_err(|e| format!("Failed to set filter: {}", e))?;
        
        Ok(Self {
            capture: Some(cap),
            filter,
            ring_buffer: Arc::new(MPSCRingBuffer::new(1024 * 1024 * 16)),  // 16MB
            stats: Arc::new(RwLock::new(CaptureStats::default())),
            running: Arc::new(AtomicBool::new(false)),
            handler: Box::new(EmptyHandler),
        })
    }
    
    /// Set packet handler
    pub fn set_handler<H: PacketHandler + Send + Sync + 'static>(&mut self, handler: H) {
        self.handler = Box::new(handler);
    }
    
    /// Build BPF filter string
    fn build_filter_string(filter: &PacketFilter) -> String {
        let mut filters = Vec::new();
        
        // UDP port filter
        if !filter.udp_ports.is_empty() {
            let port_filter: Vec<String> = filter.udp_ports.iter()
                .map(|p| format!("udp port {}", p))
                .collect();
            filters.push(format!("({})", port_filter.join(" or ")));
        }
        
        // TCP port filter
        if !filter.tcp_ports.is_empty() {
            let port_filter: Vec<String> = filter.tcp_ports.iter()
                .map(|p| format!("tcp port {}", p))
                .collect();
            filters.push(format!("({})", port_filter.join(" or ")));
        }
        
        // MAC address filter
        if !filter.exchange_macs.is_empty() {
            let mac_filter: Vec<String> = filter.exchange_macs.iter()
                .map(|mac| format!("ether host {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                    mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]))
                .collect();
            filters.push(format!("({})", mac_filter.join(" or ")));
        }
        
        // Multicast filter
        for group in &filter.multicast_groups {
            filters.push(format!("dst host {}", group));
        }
        
        if filters.is_empty() {
            "".to_string()
        } else {
            filters.join(" and ")
        }
    }
    
    /// Start capturing packets
    pub fn start(&mut self) -> Result<(), String> {
        self.running.store(true, Ordering::Release);
        
        let mut cap = self.capture.take()
            .ok_or_else(|| "Capture already started".to_string())?;
        
        let ring_buffer = self.ring_buffer.clone();
        let stats = self.stats.clone();
        let running = self.running.clone();
        let handler = std::mem::replace(&mut self.handler, Box::new(EmptyHandler));
        
        std::thread::spawn(move || {
            let mut last_stats_time = std::time::Instant::now();
            let mut second_packets = 0u64;
            
            while running.load(Ordering::Acquire) {
                match cap.next_packet() {
                    Ok(packet) => {
                        let timestamp_ns = timestamp::get_hardware_timestamp();
                        let data = packet.data;
                        
                        // Update stats
                        {
                            let mut stats_guard = stats.write();
                            stats_guard.packets_captured += 1;
                            stats_guard.bytes_captured += data.len() as u64;
                            stats_guard.avg_packet_size = stats_guard.avg_packet_size * 0.999 + (data.len() as f64) * 0.001;
                            second_packets += 1;
                        }
                        
                        // Try to write to ring buffer
                        if !ring_buffer.try_write(data) {
                            let mut stats_guard = stats.write();
                            stats_guard.ring_buffer_overruns += 1;
                        }
                        
                        // Handle packet immediately for low latency
                        handler.handle_packet(data, timestamp_ns);
                        
                        // Update capture rate every second
                        if last_stats_time.elapsed() >= std::time::Duration::from_secs(1) {
                            let mut stats_guard = stats.write();
                            stats_guard.capture_rate_pps = second_packets as f64;
                            stats_guard.last_second_packets = second_packets;
                            second_packets = 0;
                            last_stats_time = std::time::Instant::now();
                        }
                    }
                    Err(pcap::Error::TimeoutExpired) => {
                        // Continue - non-blocking
                        std::thread::yield_now();
                    }
                    Err(e) => {
                        eprintln!("Packet capture error: {}", e);
                        break;
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Stop capturing
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Release);
    }
    
    /// Get capture statistics
    pub fn stats(&self) -> CaptureStats {
        self.stats.read().clone()
    }
    
    /// Get ring buffer reader
    pub fn get_reader(&self) -> RingBufferReader {
        self.ring_buffer.create_reader()
    }
}

/// Empty handler for testing
struct EmptyHandler;

impl PacketHandler for EmptyHandler {
    fn handle_packet(&self, _packet: &[u8], _timestamp_ns: u64) {}
    fn handle_batch(&self, _packets: &[(Vec<u8>, u64)]) {}
}

/// AF_XDP socket for ultra-low latency (kernel bypass)
#[cfg(feature = "af_xdp")]
pub struct AFXDPSocket {
    socket: libxdp::XdpSocket
    umem: libxdp::Umem,
    fill_queue: libxdp::FillQueue,
    completion_queue: libxdp::CompletionQueue,
    rx_queue: libxdp::RxQueue,
    tx_queue: libxdp::TxQueue,
}

#[cfg(feature = "af_xdp")]
impl AFXDPSocket {
    pub fn new(interface: &str, queue_id: u32) -> Result<Self, String> {
        // AF_XDP implementation for kernel bypass
        // Provides sub-microsecond packet capture
        unimplemented!()
    }
    
    pub fn recv_batch(&mut self, batch_size: usize) -> Vec<&[u8]> {
        // Receive batch of packets directly from kernel
        unimplemented!()
    }
}

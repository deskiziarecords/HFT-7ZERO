pub struct LatencyWatchdog;
pub struct WatchdogConfig;
impl LatencyWatchdog {
    pub fn new(_cfg: WatchdogConfig) -> Self { Self }
    pub fn record_latency(&self, _op: &str, _lat: u64) {}
}
impl Default for WatchdogConfig { fn default() -> Self { Self } }
pub struct LatencyBreach;

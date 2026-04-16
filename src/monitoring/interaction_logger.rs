use dashmap::DashMap;
use std::sync::Arc;
use crate::os::UpdateFrequency;

pub struct InteractionLogger {
    interactions: Arc<DashMap<(String, String), u64>>,
}

impl InteractionLogger {
    pub fn new() -> Self {
        Self {
            interactions: Arc::new(DashMap::new()),
        }
    }

    pub fn log(&self, source: &str, target: &str, frequency: UpdateFrequency) {
        let key = (source.to_string(), target.to_string());
        let mut entry = self.interactions.entry(key).or_insert(0);
        *entry += 1;

        tracing::debug!("[{}] <- {} ({:?})", target, source, frequency);
    }
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CompilerState { pub value: f64 }
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryState { pub value: f64 }
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AmplifierState { pub value: f64 }
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PrivilegedState { pub value: f64 }
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InterruptState { pub value: f64 }
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CollectorState { pub value: f64 }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StateVector {
    pub privileged: PrivilegedState,
    pub compiler: CompilerState,
    pub memory: MemoryState,
    pub interrupt: InterruptState,
    pub collector: CollectorState,
    pub amplifier: AmplifierState,
}

impl StateVector {
    pub fn new(
        privileged: PrivilegedState,
        compiler: CompilerState,
        memory: MemoryState,
        interrupt: InterruptState,
        collector: CollectorState,
        amplifier: AmplifierState,
    ) -> Self {
        Self { privileged, compiler, memory, interrupt, collector, amplifier }
    }

    pub fn encode(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

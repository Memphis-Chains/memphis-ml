//! Hardware Abstraction Layer for ML backends.
//!
//! Provides trait-based abstractions for gates, sensors, and actuators
//! with a composite machine for delegating to multiple backends.

pub mod mock;

#[derive(Debug, thiserror::Error)]
pub enum HardwareError {
    #[error("gate not found: {0}")]
    GateNotFound(String),
    #[error("sensor not found: {0}")]
    SensorNotFound(String),
    #[error("actuator not found: {0}")]
    ActuatorNotFound(String),
    #[error("I/O error: {0}")]
    Io(String),
    #[error("timeout")]
    Timeout,
    #[error("hardware unavailable: {0}")]
    Unavailable(String),
}

/// Gate state: Off (0), On (1), or Toggle (flip current).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum GateState {
    Off = 0,
    On = 1,
    Toggle,
}

/// Kind of sensor capability.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SensorKind {
    Temperature,
    Humidity,
    Bool,
    Custom(String),
}

/// Gate trait — digital on/off control with state inspection.
pub trait Gate {
    /// Check whether this gate owns the given id (read-only, no mutable borrow needed).
    fn owns(&self, id: &str) -> bool;
    fn set(&mut self, id: &str, state: GateState) -> Result<(), HardwareError>;
    fn get(&mut self, id: &str) -> Result<GateState, HardwareError>;
}

/// Sensor trait — read environmental or binary values.
pub trait Sensor {
    /// Read temperature in Celsius.
    fn read_temp(&mut self, id: &str) -> Result<f64, HardwareError>;
    /// Read relative humidity as 0–100%.
    fn read_humidity(&mut self, id: &str) -> Result<f64, HardwareError>;
    /// Read a generic boolean value (0/1).
    fn read_bool(&mut self, id: &str) -> Result<bool, HardwareError>;
    /// Check whether a given sensor id supports the requested kind.
    fn supports(&self, id: &str, kind: SensorKind) -> bool;
}

/// Actuator trait — variable power control (0.0–1.0).
pub trait Actuator {
    /// Check whether this actuator owns the given id (read-only, no mutable borrow needed).
    fn owns(&self, id: &str) -> bool;
    fn set_power(&mut self, id: &str, level: f64) -> Result<(), HardwareError>;
    fn get_power(&mut self, id: &str) -> Result<f64, HardwareError>;
}

/// Composite machine — aggregates multiple gate, sensor, and actuator
/// implementations behind a unified interface.
pub struct CompositeMachine {
    gates: Vec<Box<dyn Gate + Send>>,
    sensors: Vec<Box<dyn Sensor + Send>>,
    actuators: Vec<Box<dyn Actuator + Send>>,
}

impl CompositeMachine {
    pub fn new() -> Self {
        Self { gates: Vec::new(), sensors: Vec::new(), actuators: Vec::new() }
    }

    pub fn with_gate(mut self, gate: Box<dyn Gate + Send>) -> Self {
        self.gates.push(gate); self
    }

    pub fn with_sensor(mut self, sensor: Box<dyn Sensor + Send>) -> Self {
        self.sensors.push(sensor); self
    }

    pub fn with_actuator(mut self, actuator: Box<dyn Actuator + Send>) -> Self {
        self.actuators.push(actuator); self
    }

    fn find_gate_index(&self, id: &str) -> Option<usize> {
        self.gates.iter().position(|g| g.owns(id))
    }

    fn find_sensor_index(&self, id: &str) -> Option<usize> {
        if let Some(idx) = self.sensors.iter().position(|s| s.supports(id, SensorKind::Temperature)) {
            return Some(idx);
        }
        if let Some(idx) = self.sensors.iter().position(|s| s.supports(id, SensorKind::Humidity)) {
            return Some(idx);
        }
        self.sensors.iter().position(|s| s.supports(id, SensorKind::Bool))
    }

    fn find_actuator_index(&self, id: &str) -> Option<usize> {
        self.actuators.iter().position(|a| a.owns(id))
    }
}

impl Default for CompositeMachine {
    fn default() -> Self { Self::new() }
}

impl Gate for CompositeMachine {
    fn owns(&self, id: &str) -> bool { self.gates.iter().any(|g| g.owns(id)) }
    fn set(&mut self, id: &str, state: GateState) -> Result<(), HardwareError> {
        let idx = self.find_gate_index(id).ok_or_else(|| HardwareError::GateNotFound(id.to_string()))?;
        self.gates[idx].set(id, state)
    }
    fn get(&mut self, id: &str) -> Result<GateState, HardwareError> {
        let idx = self.find_gate_index(id).ok_or_else(|| HardwareError::GateNotFound(id.to_string()))?;
        self.gates[idx].get(id)
    }
}

impl Sensor for CompositeMachine {
    fn read_temp(&mut self, id: &str) -> Result<f64, HardwareError> {
        let idx = self.find_sensor_index(id).ok_or_else(|| HardwareError::SensorNotFound(id.to_string()))?;
        self.sensors[idx].read_temp(id)
    }
    fn read_humidity(&mut self, id: &str) -> Result<f64, HardwareError> {
        let idx = self.find_sensor_index(id).ok_or_else(|| HardwareError::SensorNotFound(id.to_string()))?;
        self.sensors[idx].read_humidity(id)
    }
    fn read_bool(&mut self, id: &str) -> Result<bool, HardwareError> {
        let idx = self.find_sensor_index(id).ok_or_else(|| HardwareError::SensorNotFound(id.to_string()))?;
        self.sensors[idx].read_bool(id)
    }
    fn supports(&self, id: &str, kind: SensorKind) -> bool {
        self.sensors.iter().any(|s| s.supports(id, kind.clone()))
    }
}

impl Actuator for CompositeMachine {
    fn owns(&self, id: &str) -> bool { self.actuators.iter().any(|a| a.owns(id)) }
    fn set_power(&mut self, id: &str, level: f64) -> Result<(), HardwareError> {
        let idx = self.find_actuator_index(id).ok_or_else(|| HardwareError::ActuatorNotFound(id.to_string()))?;
        self.actuators[idx].set_power(id, level)
    }
    fn get_power(&mut self, id: &str) -> Result<f64, HardwareError> {
        let idx = self.find_actuator_index(id).ok_or_else(|| HardwareError::ActuatorNotFound(id.to_string()))?;
        self.actuators[idx].get_power(id)
    }
}

//! Mock HAL implementation for testing.

use std::collections::HashMap;

use crate::{Actuator, Gate, GateState, HardwareError, Sensor, SensorKind};

/// Mock hardware abstraction layer backed by in-memory HashMaps.
pub struct MockHAL {
    gates: HashMap<String, GateState>,
    temps: HashMap<String, f64>,
    humidity: HashMap<String, f64>,
    bools: HashMap<String, bool>,
    powers: HashMap<String, f64>,
}

impl MockHAL {
    pub fn new() -> Self {
        Self {
            gates: HashMap::new(),
            temps: HashMap::new(),
            humidity: HashMap::new(),
            bools: HashMap::new(),
            powers: HashMap::new(),
        }
    }

    /// Insert a gate with an initial state.
    pub fn with_gate(mut self, id: &str, state: GateState) -> Self {
        self.gates.insert(id.to_string(), state);
        self
    }

    /// Insert a temperature reading.
    pub fn with_temp(mut self, id: &str, temp: f64) -> Self {
        self.temps.insert(id.to_string(), temp);
        self
    }

    /// Insert a humidity reading.
    pub fn with_humidity(mut self, id: &str, humidity: f64) -> Self {
        self.humidity.insert(id.to_string(), humidity);
        self
    }

    /// Insert a boolean sensor reading.
    pub fn with_bool(mut self, id: &str, value: bool) -> Self {
        self.bools.insert(id.to_string(), value);
        self
    }

    /// Insert an actuator power level.
    pub fn with_power(mut self, id: &str, level: f64) -> Self {
        self.powers.insert(id.to_string(), level);
        self
    }
}

impl Default for MockHAL {
    fn default() -> Self {
        Self::new()
    }
}

impl Gate for MockHAL {
    fn owns(&self, id: &str) -> bool {
        self.gates.contains_key(id)
    }

    fn set(&mut self, id: &str, state: GateState) -> Result<(), HardwareError> {
        if !self.gates.contains_key(id) {
            return Err(HardwareError::GateNotFound(id.to_string()));
        }
        let resolved = match state {
            GateState::Toggle => {
                let current = self.gates.get(id).copied().unwrap_or(GateState::Off);
                if current == GateState::On {
                    GateState::Off
                } else {
                    GateState::On
                }
            }
            _ => state,
        };
        self.gates.insert(id.to_string(), resolved);
        Ok(())
    }

    fn get(&mut self, id: &str) -> Result<GateState, HardwareError> {
        self.gates
            .get(id)
            .copied()
            .ok_or_else(|| HardwareError::GateNotFound(id.to_string()))
    }
}

impl Sensor for MockHAL {
    fn read_temp(&mut self, id: &str) -> Result<f64, HardwareError> {
        self.temps
            .get(id)
            .copied()
            .ok_or_else(|| HardwareError::SensorNotFound(id.to_string()))
    }

    fn read_humidity(&mut self, id: &str) -> Result<f64, HardwareError> {
        self.humidity
            .get(id)
            .copied()
            .ok_or_else(|| HardwareError::SensorNotFound(id.to_string()))
    }

    fn read_bool(&mut self, id: &str) -> Result<bool, HardwareError> {
        self.bools
            .get(id)
            .copied()
            .ok_or_else(|| HardwareError::SensorNotFound(id.to_string()))
    }

    fn supports(&self, id: &str, kind: SensorKind) -> bool {
        match kind {
            SensorKind::Temperature => self.temps.contains_key(id),
            SensorKind::Humidity => self.humidity.contains_key(id),
            SensorKind::Bool => self.bools.contains_key(id),
            SensorKind::Custom(_) => false,
        }
    }
}

impl Actuator for MockHAL {
    fn owns(&self, id: &str) -> bool {
        self.powers.contains_key(id)
    }

    fn set_power(&mut self, id: &str, level: f64) -> Result<(), HardwareError> {
        if !self.powers.contains_key(id) {
            return Err(HardwareError::ActuatorNotFound(id.to_string()));
        }
        if !(0.0..=1.0).contains(&level) {
            return Err(HardwareError::Io("power level must be between 0.0 and 1.0".to_string()));
        }
        self.powers.insert(id.to_string(), level);
        Ok(())
    }

    fn get_power(&mut self, id: &str) -> Result<f64, HardwareError> {
        self.powers
            .get(id)
            .copied()
            .ok_or_else(|| HardwareError::ActuatorNotFound(id.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gate_on_off() {
        let mut hal = MockHAL::new().with_gate("led1", GateState::Off);
        assert_eq!(hal.get("led1").unwrap(), GateState::Off);
        hal.set("led1", GateState::On).unwrap();
        assert_eq!(hal.get("led1").unwrap(), GateState::On);
    }

    #[test]
    fn test_gate_toggle() {
        let mut hal = MockHAL::new().with_gate("btn1", GateState::Off);
        hal.set("btn1", GateState::Toggle).unwrap();
        assert_eq!(hal.get("btn1").unwrap(), GateState::On);
        hal.set("btn1", GateState::Toggle).unwrap();
        assert_eq!(hal.get("btn1").unwrap(), GateState::Off);
    }

    #[test]
    fn test_sensor_temp() {
        let mut hal = MockHAL::new().with_temp("temp1", 25.5);
        assert_eq!(hal.read_temp("temp1").unwrap(), 25.5);
    }

    #[test]
    fn test_sensor_not_found() {
        let mut hal = MockHAL::new();
        assert!(matches!(hal.read_temp("missing"), Err(HardwareError::SensorNotFound(_))));
    }

    #[test]
    fn test_actuator_power() {
        let mut hal = MockHAL::new().with_power("fan1", 0.5);
        assert_eq!(hal.get_power("fan1").unwrap(), 0.5);
        hal.set_power("fan1", 0.8).unwrap();
        assert_eq!(hal.get_power("fan1").unwrap(), 0.8);
    }

    #[test]
    fn test_actuator_out_of_range() {
        let mut hal = MockHAL::new().with_power("fan1", 0.5);
        let result = hal.set_power("fan1", 1.5);
        assert!(matches!(result, Err(HardwareError::Io(_))));
    }

    #[test]
    fn test_sensor_supports() {
        let hal = MockHAL::new()
            .with_temp("temp1", 20.0)
            .with_humidity("hum1", 60.0)
            .with_bool("btn1", true);

        assert!(hal.supports("temp1", SensorKind::Temperature));
        assert!(!hal.supports("temp1", SensorKind::Humidity));
        assert!(hal.supports("hum1", SensorKind::Humidity));
        assert!(hal.supports("btn1", SensorKind::Bool));
        assert!(!hal.supports("btn1", SensorKind::Temperature));
    }

    #[test]
    fn test_gate_not_found() {
        let mut hal = MockHAL::new();
        assert!(matches!(hal.get("missing"), Err(HardwareError::GateNotFound(_))));
    }
}

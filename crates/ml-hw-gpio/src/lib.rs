//! GPIO hardware backend using Linux sysfs.
//!
//! This crate provides a [`SysfsGpio`] implementation for controlling real GPIO pins
//! on Linux/Raspberry Pi systems via the `/sys/class/gpio` interface.
//!
//! # Features
//!
//! - Export/unexport GPIO pins
//! - Set pin direction (input, output, output-high, output-low)
//! - Read/write pin values
//! - Implementations of `ml_hal::Gate` and `ml_hal::Sensor` traits
//!
//! # Example
//!
//! ```rust,no_run
//! use ml_hw_gpio::SysfsGpio;
//!
//! let mut gpio = SysfsGpio::new("/sys/class/gpio").unwrap();
//! gpio.configure_output(17).unwrap();
//! gpio.write_value(17, 1).unwrap(); // Turn on
//! ```

pub mod sysfs;

pub use sysfs::{Direction, GpioError, SysfsGpio};

use std::path::PathBuf;

/// Gate trait implementation for SysfsGpio.
///
/// This allows using GPIO pins as on/off switches (gates) controlled by the ml-hal interface.
/// Pin naming convention: "gpio17" maps to `/sys/class/gpio/gpio17`
impl ml_hal::Gate for SysfsGpio {
    fn owns(&self, id: &str) -> bool {
        // This GPIO instance "owns" all gpioN pin names
        id.starts_with("gpio") && parse_pin_name(id).is_ok()
    }

    fn set(&mut self, pin_name: &str, state: ml_hal::GateState) -> Result<(), ml_hal::HardwareError> {
        // Parse pin name: "gpio17" -> 17
        let pin = parse_pin_name(pin_name)?;

        // Ensure pin is exported and configured as output
        if !self.is_exported(pin) {
            self.export_pin(pin)
                .map_err(|e| ml_hal::HardwareError::Io(e.to_string()))?;
        }
        self.set_direction(pin, sysfs::Direction::Out)
            .map_err(|e| ml_hal::HardwareError::Io(e.to_string()))?;

        match state {
            ml_hal::GateState::On => self.write_value(pin, 1),
            ml_hal::GateState::Off => self.write_value(pin, 0),
            ml_hal::GateState::Toggle => {
                let current = self.read_value(pin)
                    .map_err(|e| ml_hal::HardwareError::Io(e.to_string()))?;
                let new_value = if current == 0 { 1 } else { 0 };
                self.write_value(pin, new_value)
            }
        }
        .map_err(|e| ml_hal::HardwareError::Io(e.to_string()))
    }

    fn get(&mut self, pin_name: &str) -> Result<ml_hal::GateState, ml_hal::HardwareError> {
        let pin = parse_pin_name(pin_name)?;
        let value = self.read_value(pin)
            .map_err(|e| ml_hal::HardwareError::Io(e.to_string()))?;
        Ok(if value == 1 {
            ml_hal::GateState::On
        } else {
            ml_hal::GateState::Off
        })
    }
}

/// Sensor trait implementation for SysfsGpio.
///
/// This allows reading boolean sensors (motion detectors, door sensors, etc.)
/// connected to GPIO pins. Temperature sensors would require 1-wire or similar
/// protocols and are stubbed out for now.
impl ml_hal::Sensor for SysfsGpio {
    fn read_temp(&mut self, pin_name: &str) -> Result<f64, ml_hal::HardwareError> {
        let pin = parse_pin_name(pin_name)?;
        let _ = pin; // suppress unused warning
        Err(ml_hal::HardwareError::Unavailable(
            "Temperature sensors via raw GPIO not supported. Use 1-wire interface.".into(),
        ))
    }

    fn read_humidity(&mut self, pin_name: &str) -> Result<f64, ml_hal::HardwareError> {
        let pin = parse_pin_name(pin_name)?;
        let _ = pin;
        Err(ml_hal::HardwareError::Unavailable(
            "Humidity sensors via raw GPIO not supported.".into(),
        ))
    }

    fn read_bool(&mut self, pin_name: &str) -> Result<bool, ml_hal::HardwareError> {
        let pin = parse_pin_name(pin_name)?;
        let value = self.read_value(pin)
            .map_err(|e| ml_hal::HardwareError::Io(e.to_string()))?;
        Ok(value == 1)
    }

    fn supports(&self, _pin_name: &str, kind: ml_hal::SensorKind) -> bool {
        match kind {
            ml_hal::SensorKind::Bool => true,
            ml_hal::SensorKind::Temperature => false,
            ml_hal::SensorKind::Humidity => false,
            ml_hal::SensorKind::Custom(_) => false,
        }
    }
}

/// Parse a pin name like "gpio17" into the numeric pin number.
///
/// # Errors
///
/// Returns an error if the pin name format is invalid.
fn parse_pin_name(pin_name: &str) -> Result<u32, ml_hal::HardwareError> {
    let prefix = "gpio";
    if !pin_name.starts_with(prefix) {
        return Err(ml_hal::HardwareError::Io(format!(
            "Invalid pin name '{}': expected format 'gpio{{N}}' (e.g., 'gpio17')",
            pin_name
        )));
    }

    let num_str = &pin_name[prefix.len()..];
    num_str
        .parse::<u32>()
        .map_err(|_| ml_hal::HardwareError::Io(format!("Invalid pin number in '{}'", pin_name)))
}

/// Create a numeric pin name from a pin number.
///
/// Example: `make_pin_name(17)` returns `"gpio17"`.
pub fn make_pin_name(pin: u32) -> String {
    format!("gpio{}", pin)
}

/// Builder for configuring multiple GPIO pins at once.
#[derive(Debug, Default)]
pub struct GpioBuilder {
    base_path: PathBuf,
    output_pins: Vec<u32>,
    input_pins: Vec<u32>,
}

impl GpioBuilder {
    /// Create a new builder with the given sysfs base path.
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
            output_pins: Vec::new(),
            input_pins: Vec::new(),
        }
    }

    /// Add a pin to be configured as output.
    pub fn with_output(mut self, pin: u32) -> Self {
        self.output_pins.push(pin);
        self
    }

    /// Add a pin to be configured as input.
    pub fn with_input(mut self, pin: u32) -> Self {
        self.input_pins.push(pin);
        self
    }

    /// Configure multiple pins as outputs.
    pub fn with_outputs(mut self, pins: &[u32]) -> Self {
        self.output_pins.extend_from_slice(pins);
        self
    }

    /// Configure multiple pins as inputs.
    pub fn with_inputs(mut self, pins: &[u32]) -> Self {
        self.input_pins.extend_from_slice(pins);
        self
    }

    /// Build the SysfsGpio instance and configure all pins.
    pub fn build(self) -> Result<SysfsGpio, GpioError> {
        let mut gpio = SysfsGpio::new(&self.base_path)?;

        for &pin in &self.output_pins {
            gpio.configure_output(pin)?;
        }

        for &pin in &self.input_pins {
            gpio.configure_input(pin)?;
        }

        Ok(gpio)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pin_name() {
        assert_eq!(parse_pin_name("gpio17").unwrap(), 17);
        assert_eq!(parse_pin_name("gpio0").unwrap(), 0);
        assert_eq!(parse_pin_name("gpio255").unwrap(), 255);
    }

    #[test]
    fn test_parse_pin_name_invalid() {
        assert!(parse_pin_name("gpio").is_err());
        assert!(parse_pin_name("gpioabc").is_err());
        assert!(parse_pin_name("17").is_err());
        assert!(parse_pin_name("pin17").is_err());
    }

    #[test]
    fn test_make_pin_name() {
        assert_eq!(make_pin_name(17), "gpio17");
        assert_eq!(make_pin_name(0), "gpio0");
    }

    #[test]
    fn test_gpio_builder() {
        // Just test that the builder compiles and creates proper structure
        let builder = GpioBuilder::new("/sys/class/gpio")
            .with_output(17)
            .with_output(27)
            .with_input(22);

        assert_eq!(builder.output_pins, vec![17, 27]);
        assert_eq!(builder.input_pins, vec![22]);
    }
}

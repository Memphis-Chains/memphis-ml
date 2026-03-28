//! Linux sysfs GPIO implementation.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::PathBuf;
use std::sync::Mutex;

/// Direction of a GPIO pin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Input direction.
    In,
    /// Output direction (low by default).
    Out,
    /// Output with pull-up (initial value high).
    OutHigh,
    /// Output with pull-down (initial value low).
    OutLow,
}

/// Errors that can occur when interacting with GPIO.
#[derive(Debug, thiserror::Error)]
pub enum GpioError {
    #[error("pin {0} not exported")]
    NotExported(u32),

    #[error("permission denied: {0}")]
    Permission(String),

    #[error("IO error: {0}")]
    Io(String),
}

impl From<std::io::Error> for GpioError {
    fn from(err: std::io::Error) -> Self {
        match err.kind() {
            std::io::ErrorKind::PermissionDenied => {
                GpioError::Permission(err.to_string())
            }
            _ => GpioError::Io(err.to_string()),
        }
    }
}

/// Sysfs GPIO controller.
///
/// Controls GPIO pins via the Linux sysfs interface at `/sys/class/gpio`.
#[derive(Debug)]
pub struct SysfsGpio {
    base_path: PathBuf,
    exported_pins: Mutex<HashMap<u32, PathBuf>>,
}

impl SysfsGpio {
    /// Create a new SysfsGpio controller with the given base path.
    ///
    /// Typically `base_path` is `/sys/class/gpio`.
    pub fn new(base_path: impl Into<PathBuf>) -> Result<Self, GpioError> {
        let base_path = base_path.into();
        Ok(Self {
            base_path,
            exported_pins: Mutex::new(HashMap::new()),
        })
    }

    /// Returns the path for a GPIO chip name (e.g., "gpio17" -> gpio17 directory).
    fn pin_path(&self, pin: u32) -> PathBuf {
        self.base_path.join(format!("gpio{}", pin))
    }

    /// Export a pin for use.
    ///
    /// This writes the pin number to `/sys/class/gpio/export`, which creates
    /// the `gpioN` directory.
    pub fn export_pin(&mut self, pin: u32) -> Result<(), GpioError> {
        let export_path = self.base_path.join("export");

        // Write pin number to export file
        fs::write(&export_path, pin.to_string())?;

        // Track the exported pin
        let pin_dir = self.pin_path(pin);
        let mut exported = self.exported_pins.lock().unwrap();
        exported.insert(pin, pin_dir);

        Ok(())
    }

    /// Unexport a pin.
    ///
    /// This writes the pin number to `/sys/class/gpio/unexport`.
    pub fn unexport_pin(&mut self, pin: u32) -> Result<(), GpioError> {
        let unexport_path = self.base_path.join("unexport");

        // Write pin number to unexport file
        fs::write(&unexport_path, pin.to_string())?;

        // Remove from tracking
        let mut exported = self.exported_pins.lock().unwrap();
        exported.remove(&pin);

        Ok(())
    }

    /// Check if a pin is currently exported.
    pub fn is_exported(&self, pin: u32) -> bool {
        let exported = self.exported_pins.lock().unwrap();
        exported.contains_key(&pin)
    }

    /// Get the path for an exported pin.
    fn get_pin_path(&self, pin: u32) -> Result<PathBuf, GpioError> {
        let exported = self.exported_pins.lock().unwrap();
        exported
            .get(&pin)
            .cloned()
            .ok_or(GpioError::NotExported(pin))
    }

    /// Set the direction of an exported pin.
    pub fn set_direction(&mut self, pin: u32, dir: Direction) -> Result<(), GpioError> {
        let pin_path = self.get_pin_path(pin)?;
        let dir_path = pin_path.join("direction");

        let dir_str = match dir {
            Direction::In => "in",
            Direction::Out => "out",
            Direction::OutHigh => "high",
            Direction::OutLow => "low",
        };

        fs::write(&dir_path, dir_str)?;
        Ok(())
    }

    /// Write a value to an exported pin (0 or 1).
    pub fn write_value(&mut self, pin: u32, value: u8) -> Result<(), GpioError> {
        if value > 1 {
            return Err(GpioError::Io(format!(
                "GPIO value must be 0 or 1, got {}",
                value
            )));
        }

        let pin_path = self.get_pin_path(pin)?;
        let value_path = pin_path.join("value");

        fs::write(&value_path, value.to_string())?;
        Ok(())
    }

    /// Read the current value of an exported pin (returns 0 or 1).
    pub fn read_value(&self, pin: u32) -> Result<u8, GpioError> {
        let exported = self.exported_pins.lock().unwrap();
        let pin_path = exported
            .get(&pin)
            .cloned()
            .ok_or(GpioError::NotExported(pin))?;

        let value_path = pin_path.join("value");

        let mut file = File::open(&value_path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let value = contents.trim().parse::<u8>().map_err(|e| {
            GpioError::Io(format!("failed to parse GPIO value '{}': {}", contents.trim(), e))
        })?;

        if value > 1 {
            return Err(GpioError::Io(format!("GPIO value out of range: {}", value)));
        }

        Ok(value)
    }

    /// Configure a pin as output.
    ///
    /// Exports the pin if not already exported, then sets direction to "out".
    pub fn configure_output(&mut self, pin: u32) -> Result<(), GpioError> {
        if !self.is_exported(pin) {
            self.export_pin(pin)?;
        }
        self.set_direction(pin, Direction::Out)
    }

    /// Configure a pin as input.
    ///
    /// Exports the pin if not already exported, then sets direction to "in".
    pub fn configure_input(&mut self, pin: u32) -> Result<(), GpioError> {
        if !self.is_exported(pin) {
            self.export_pin(pin)?;
        }
        self.set_direction(pin, Direction::In)
    }

    /// Toggle an output pin (flip current value).
    ///
    /// Returns the new value after toggling.
    pub fn toggle(&mut self, pin: u32) -> Result<u8, GpioError> {
        let current = self.read_value(pin)?;
        let new_value = if current == 0 { 1 } else { 0 };
        self.write_value(pin, new_value)?;
        Ok(new_value)
    }
}

// Trait for mockability - provides a trait interface over SysfsGpio operations
/// Trait for GPIO pin operations - can be implemented for mocking in tests.
pub trait GpioPin {
    fn read(&self) -> Result<u8, GpioError>;
    fn write(&mut self, value: u8) -> Result<(), GpioError>;
    fn set_direction(&mut self, dir: Direction) -> Result<(), GpioError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Mockable GPIO pin for testing without hardware.
    pub struct MockGpioPin {
        pub value: Mutex<u8>,
        pub direction: Mutex<Direction>,
        exported: bool,
    }

    impl Default for MockGpioPin {
        fn default() -> Self {
            Self {
                value: Mutex::new(0),
                direction: Mutex::new(Direction::Out),
                exported: false,
            }
        }
    }

    impl std::fmt::Debug for MockGpioPin {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("MockGpioPin")
                .field("value", &*self.value.lock().unwrap())
                .field("direction", &*self.direction.lock().unwrap())
                .field("exported", &self.exported)
                .finish()
        }
    }

    impl MockGpioPin {
        pub fn new() -> Self {
            Self::default()
        }
    }

    impl GpioPin for MockGpioPin {
        fn read(&self) -> Result<u8, GpioError> {
            Ok(*self.value.lock().unwrap())
        }

        fn write(&mut self, value: u8) -> Result<(), GpioError> {
            if value > 1 {
                return Err(GpioError::Io("Value must be 0 or 1".into()));
            }
            *self.value.lock().unwrap() = value;
            Ok(())
        }

        fn set_direction(&mut self, dir: Direction) -> Result<(), GpioError> {
            *self.direction.lock().unwrap() = dir;
            Ok(())
        }
    }

    #[test]
    fn test_mock_gpio_basic() {
        let mut pin = MockGpioPin::new();
        pin.write(1).unwrap();
        assert_eq!(pin.read().unwrap(), 1);
        pin.write(0).unwrap();
        assert_eq!(pin.read().unwrap(), 0);
    }
}

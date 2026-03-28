//! Simple example demonstrating GPIO control on a Raspberry Pi.
//!
//! This example toggles GPIO 17 (pin 11 on the header) on and off.
//!
//! # Prerequisites
//!
//! - Run as root or with appropriate GPIO permissions
//! - The sysfs GPIO interface must be enabled
//!
//! # Usage
//!
//! ```bash
//! cargo run --example simple
//! ```

use ml_hw_gpio::SysfsGpio;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a new GPIO controller pointing to sysfs
    let mut gpio = SysfsGpio::new("/sys/class/gpio")?;

    // Configure pin 17 as an output
    println!("Configuring GPIO 17 as output...");
    gpio.configure_output(17)?;

    // Turn on (set to high)
    println!("Turning GPIO 17 ON...");
    gpio.write_value(17, 1)?;

    // Wait for a second
    std::thread::sleep(std::time::Duration::from_secs(1));

    // Turn off (set to low)
    println!("Turning GPIO 17 OFF...");
    gpio.write_value(17, 0)?;

    println!("Done! GPIO 17 has been toggled.");

    // Cleanup: unexport the pin
    gpio.unexport_pin(17)?;
    println!("Pin 17 unexported.");

    Ok(())
}

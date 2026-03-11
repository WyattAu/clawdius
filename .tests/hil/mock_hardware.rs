//! Mock Hardware Interface for HIL Testing
//!
//! This module provides mock implementations of hardware interfaces
//! for CI testing without real hardware.

use rand::Rng;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Configuration for mock hardware behavior
#[derive(Debug, Clone)]
pub struct MockConfig {
    /// Simulate realistic delays
    pub simulate_delays: bool,
    /// Minimum simulated delay in milliseconds
    pub min_delay_ms: u64,
    /// Maximum simulated delay in milliseconds
    pub max_delay_ms: u64,
    /// Inject random failures
    pub inject_failures: bool,
    /// Failure rate (0.0 to 1.0)
    pub failure_rate: f64,
}

impl Default for MockConfig {
    fn default() -> Self {
        Self {
            simulate_delays: true,
            min_delay_ms: 1,
            max_delay_ms: 50,
            inject_failures: false,
            failure_rate: 0.0,
        }
    }
}

/// Mock sensor data generator
#[derive(Debug, Clone)]
pub struct MockSensor {
    name: String,
    min_value: f64,
    max_value: f64,
    unit: String,
    current_value: f64,
    history: VecDeque<f64>,
    max_history: usize,
}

impl MockSensor {
    pub fn new(name: impl Into<String>, min: f64, max: f64, unit: impl Into<String>) -> Self {
        let mut rng = rand::rng();
        let initial = rng.random_range(min..max);
        Self {
            name: name.into(),
            min_value: min,
            max_value: max,
            unit: unit.into(),
            current_value: initial,
            history: VecDeque::with_capacity(100),
            max_history: 100,
        }
    }

    pub fn read(&mut self) -> f64 {
        let mut rng = rand::rng();
        let delta = (self.max_value - self.min_value) * 0.1;
        let change = rng.random_range(-delta..delta);
        self.current_value = (self.current_value + change).clamp(self.min_value, self.max_value);

        self.history.push_back(self.current_value);
        if self.history.len() > self.max_history {
            self.history.pop_front();
        }

        self.current_value
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn unit(&self) -> &str {
        &self.unit
    }

    pub fn history(&self) -> &VecDeque<f64> {
        &self.history
    }
}

/// Mock GPIO pin state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GpioState {
    Low,
    High,
    Input,
    Output,
    Pwm(u16),
}

/// Mock GPIO pin
pub struct MockGpioPin {
    pin: u8,
    state: GpioState,
    value: bool,
    pwm_duty: u16,
    pwm_frequency: u32,
}

impl MockGpioPin {
    pub fn new(pin: u8) -> Self {
        Self {
            pin,
            state: GpioState::Input,
            value: false,
            pwm_duty: 0,
            pwm_frequency: 0,
        }
    }

    pub fn set_mode(&mut self, state: GpioState) {
        self.state = state;
    }

    pub fn write(&mut self, value: bool) -> Result<(), MockHardwareError> {
        if self.state != GpioState::Output {
            return Err(MockHardwareError::InvalidPinMode {
                pin: self.pin,
                expected: "output",
                actual: format!("{:?}", self.state),
            });
        }
        self.value = value;
        Ok(())
    }

    pub fn read(&self) -> bool {
        self.value
    }

    pub fn set_pwm(&mut self, duty: u16, frequency: u32) -> Result<(), MockHardwareError> {
        if !matches!(self.state, GpioState::Pwm(_)) {
            self.state = GpioState::Pwm(duty);
        }
        self.pwm_duty = duty;
        self.pwm_frequency = frequency;
        Ok(())
    }
}

/// Mock serial port
pub struct MockSerialPort {
    name: String,
    baud_rate: u32,
    tx_buffer: VecDeque<u8>,
    rx_buffer: VecDeque<u8>,
    is_open: bool,
}

impl MockSerialPort {
    pub fn new(name: impl Into<String>, baud_rate: u32) -> Self {
        Self {
            name: name.into(),
            baud_rate,
            tx_buffer: VecDeque::with_capacity(4096),
            rx_buffer: VecDeque::with_capacity(4096),
            is_open: false,
        }
    }

    pub fn open(&mut self) -> Result<(), MockHardwareError> {
        self.is_open = true;
        Ok(())
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn write(&mut self, data: &[u8]) -> Result<usize, MockHardwareError> {
        if !self.is_open {
            return Err(MockHardwareError::PortClosed(self.name.clone()));
        }
        for &byte in data {
            self.tx_buffer.push_back(byte);
        }
        Ok(data.len())
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, MockHardwareError> {
        if !self.is_open {
            return Err(MockHardwareError::PortClosed(self.name.clone()));
        }
        let mut count = 0;
        for byte in buf.iter_mut() {
            if let Some(b) = self.rx_buffer.pop_front() {
                *byte = b;
                count += 1;
            } else {
                break;
            }
        }
        Ok(count)
    }

    pub fn inject_rx_data(&mut self, data: &[u8]) {
        for &byte in data {
            self.rx_buffer.push_back(byte);
        }
    }

    pub fn drain_tx(&mut self) -> Vec<u8> {
        self.tx_buffer.drain(..).collect()
    }
}

/// Mock SPI bus
pub struct MockSpiBus {
    clock_speed: u32,
    mode: u8,
    tx_data: Vec<u8>,
    rx_data: Vec<u8>,
}

impl MockSpiBus {
    pub fn new(clock_speed: u32, mode: u8) -> Self {
        Self {
            clock_speed,
            mode,
            tx_data: Vec::new(),
            rx_data: Vec::new(),
        }
    }

    pub fn transfer(&mut self, tx: &[u8]) -> Result<Vec<u8>, MockHardwareError> {
        self.tx_data.extend_from_slice(tx);
        let rx = self.rx_data.clone();
        self.rx_data.clear();
        Ok(rx)
    }

    pub fn set_rx_response(&mut self, data: &[u8]) {
        self.rx_data = data.to_vec();
    }
}

/// Mock I2C bus
pub struct MockI2CBus {
    devices: std::collections::HashMap<u8, Vec<u8>>,
}

impl MockI2CBus {
    pub fn new() -> Self {
        Self {
            devices: std::collections::HashMap::new(),
        }
    }

    pub fn write(&mut self, addr: u8, data: &[u8]) -> Result<(), MockHardwareError> {
        let device = self.devices.entry(addr).or_insert_with(Vec::new);
        device.extend_from_slice(data);
        Ok(())
    }

    pub fn read(&mut self, addr: u8, len: usize) -> Result<Vec<u8>, MockHardwareError> {
        let device = self
            .devices
            .get_mut(&addr)
            .ok_or(MockHardwareError::I2CDeviceNotFound(addr))?;
        let result: Vec<u8> = device.drain(..len.min(device.len())).collect();
        Ok(result)
    }

    pub fn register_device(&mut self, addr: u8, initial_data: Vec<u8>) {
        self.devices.insert(addr, initial_data);
    }
}

/// Mock hardware system
pub struct MockHardware {
    config: MockConfig,
    gpio_pins: std::collections::HashMap<u8, MockGpioPin>,
    serial_ports: std::collections::HashMap<String, MockSerialPort>,
    spi_buses: Vec<MockSpiBus>,
    i2c_bus: MockI2CBus,
    sensors: std::collections::HashMap<String, MockSensor>,
    uptime: Arc<AtomicU64>,
    running: Arc<AtomicBool>,
    operation_count: Arc<AtomicU64>,
}

impl MockHardware {
    pub fn new(config: MockConfig) -> Self {
        Self {
            config,
            gpio_pins: std::collections::HashMap::new(),
            serial_ports: std::collections::HashMap::new(),
            spi_buses: Vec::new(),
            i2c_bus: MockI2CBus::new(),
            sensors: std::collections::HashMap::new(),
            uptime: Arc::new(AtomicU64::new(0)),
            running: Arc::new(AtomicBool::new(true)),
            operation_count: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn with_default_sensors(mut self) -> Self {
        self.sensors.insert(
            "temperature".to_string(),
            MockSensor::new("temperature", 20.0, 80.0, "celsius"),
        );
        self.sensors.insert(
            "voltage".to_string(),
            MockSensor::new("voltage", 3.0, 5.0, "volts"),
        );
        self.sensors.insert(
            "current".to_string(),
            MockSensor::new("current", 0.0, 2.0, "amps"),
        );
        self
    }

    fn simulate_delay(&self) {
        if self.config.simulate_delays {
            let mut rng = rand::rng();
            let delay_ms = rng.random_range(self.config.min_delay_ms..=self.config.max_delay_ms);
            std::thread::sleep(Duration::from_millis(delay_ms));
        }
    }

    fn check_failure(&self) -> Result<(), MockHardwareError> {
        if self.config.inject_failures {
            let mut rng = rand::rng();
            if rng.random::<f64>() < self.config.failure_rate {
                return Err(MockHardwareError::InjectedFailure);
            }
        }
        self.operation_count.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    pub fn gpio(&mut self, pin: u8) -> &mut MockGpioPin {
        self.gpio_pins
            .entry(pin)
            .or_insert_with(|| MockGpioPin::new(pin))
    }

    pub fn serial(&mut self, name: &str, baud_rate: u32) -> &mut MockSerialPort {
        self.serial_ports
            .entry(name.to_string())
            .or_insert_with(|| MockSerialPort::new(name, baud_rate))
    }

    pub fn spi(&mut self, index: usize) -> Option<&mut MockSpiBus> {
        self.spi_buses.get_mut(index)
    }

    pub fn add_spi_bus(&mut self, clock_speed: u32, mode: u8) -> usize {
        let index = self.spi_buses.len();
        self.spi_buses.push(MockSpiBus::new(clock_speed, mode));
        index
    }

    pub fn i2c(&mut self) -> &mut MockI2CBus {
        &mut self.i2c_bus
    }

    pub fn read_sensor(&mut self, name: &str) -> Result<f64, MockHardwareError> {
        self.simulate_delay();
        self.check_failure()?;
        self.sensors
            .get_mut(name)
            .map(|s| s.read())
            .ok_or(MockHardwareError::SensorNotFound(name.to_string()))
    }

    pub fn uptime_ms(&self) -> u64 {
        self.uptime.load(Ordering::Relaxed)
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub fn operation_count(&self) -> u64 {
        self.operation_count.load(Ordering::Relaxed)
    }

    pub fn shutdown(&self) {
        self.running.store(false, Ordering::Relaxed);
    }

    pub fn tick(&self) {
        self.uptime.fetch_add(1, Ordering::Relaxed);
    }
}

/// Errors that can occur during mock hardware operations
#[derive(Debug, thiserror::Error)]
pub enum MockHardwareError {
    #[error("Invalid pin mode for pin {pin}: expected {expected}, got {actual}")]
    InvalidPinMode {
        pin: u8,
        expected: &'static str,
        actual: String,
    },
    #[error("Serial port '{0}' is not open")]
    PortClosed(String),
    #[error("I2C device not found at address {0:#x}")]
    I2CDeviceNotFound(u8),
    #[error("Sensor '{0}' not found")]
    SensorNotFound(String),
    #[error("Injected failure for testing")]
    InjectedFailure,
    #[error("Timeout waiting for operation")]
    Timeout,
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

/// Test context for HIL tests
pub struct HilTestContext {
    hardware: MockHardware,
    start_time: Instant,
    test_name: String,
}

impl HilTestContext {
    pub fn new(test_name: impl Into<String>, config: MockConfig) -> Self {
        Self {
            hardware: MockHardware::new(config).with_default_sensors(),
            start_time: Instant::now(),
            test_name: test_name.into(),
        }
    }

    pub fn hardware(&mut self) -> &mut MockHardware {
        &mut self.hardware
    }

    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    pub fn test_name(&self) -> &str {
        &self.test_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_gpio() {
        let mut hw = MockHardware::new(MockConfig::default());
        hw.gpio(5).set_mode(GpioState::Output);
        hw.gpio(5).write(true).unwrap();
        assert!(hw.gpio(5).read());
    }

    #[test]
    fn test_mock_sensor() {
        let mut hw = MockHardware::new(MockConfig::default()).with_default_sensors();
        let temp = hw.read_sensor("temperature").unwrap();
        assert!(temp >= 20.0 && temp <= 80.0);
    }

    #[test]
    fn test_mock_serial() {
        let mut hw = MockHardware::new(MockConfig::default());
        hw.serial("ttyUSB0", 115200).open().unwrap();
        hw.serial("ttyUSB0", 115200)
            .write(&[0x01, 0x02, 0x03])
            .unwrap();
        let tx = hw.serial("ttyUSB0", 115200).drain_tx();
        assert_eq!(tx, vec![0x01, 0x02, 0x03]);
    }

    #[test]
    fn test_mock_i2c() {
        let mut hw = MockHardware::new(MockConfig::default());
        hw.i2c().register_device(0x50, vec![0xDE, 0xAD, 0xBE, 0xEF]);
        let data = hw.i2c().read(0x50, 2).unwrap();
        assert_eq!(data, vec![0xDE, 0xAD]);
    }
}

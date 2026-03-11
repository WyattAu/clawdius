# Hardware-in-the-Loop (HIL) Test Plan

## Overview

This document outlines the Hardware-in-the-Loop testing strategy for Clawdius. HIL testing enables verification of software behavior with hardware interfaces without requiring physical hardware during CI.

## Objectives

1. Validate hardware abstraction layer correctness
2. Test hardware failure handling and recovery
3. Verify timing-sensitive operations
4. Enable CI testing without physical hardware
5. Provide regression testing for hardware interactions

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     HIL Test Framework                       │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ Test Runner │  │ Config Mgr  │  │  Report Generator   │  │
│  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘  │
│         │                │                     │             │
│         └────────────────┼─────────────────────┘             │
│                          ▼                                   │
│  ┌───────────────────────────────────────────────────────┐   │
│  │              Hardware Abstraction Layer                │   │
│  └───────────────────────────┬───────────────────────────┘   │
│                              │                               │
│              ┌───────────────┴───────────────┐               │
│              ▼                               ▼               │
│  ┌───────────────────┐           ┌───────────────────────┐   │
│  │   Mock Hardware   │           │    Real Hardware      │   │
│  │   (CI Testing)    │           │   (Lab Testing)       │   │
│  └───────────────────┘           └───────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

## Test Categories

### 1. Unit Tests (`hil_unit_*`)

Test individual hardware abstraction components in isolation.

| Test ID | Description | Timeout |
|---------|-------------|---------|
| hil_unit_gpio | GPIO read/write operations | 30s |
| hil_unit_spi | SPI bus transfers | 30s |
| hil_unit_i2c | I2C device communication | 30s |
| hil_unit_serial | Serial port operations | 30s |
| hil_unit_sensor | Sensor data reading | 30s |

### 2. Integration Tests (`hil_integration_*`)

Test hardware interactions with the broader system.

| Test ID | Description | Timeout |
|---------|-------------|---------|
| hil_integration_boot | Hardware initialization sequence | 60s |
| hil_integration_multi_bus | Multi-bus concurrent operations | 60s |
| hil_integration_state | State persistence across operations | 60s |

### 3. Stress Tests (`hil_stress_*`)

Test system behavior under heavy load.

| Test ID | Description | Iterations |
|---------|-------------|------------|
| hil_stress_gpio | Rapid GPIO toggling | 10,000 |
| hil_stress_spi | Continuous SPI transfers | 5,000 |
| hil_stress_mixed | Mixed operations | 1,000 |

### 4. Safety Tests (`hil_safety_*`)

Test safety-critical behaviors.

| Test ID | Description | Timeout |
|---------|-------------|---------|
| hil_safety_timeout | Operation timeout handling | 120s |
| hil_safety_recovery | Failure recovery | 120s |
| hil_safety_watchdog | Watchdog timer behavior | 120s |

## Mock Hardware Capabilities

### Simulated Components

1. **GPIO**
   - Configurable pin modes (input, output, PWM)
   - State tracking and history
   - Simulated timing delays

2. **Serial Ports**
   - TX/RX buffer simulation
   - Configurable baud rates
   - Data injection for testing

3. **SPI Bus**
   - Full-duplex transfer simulation
   - Configurable clock speed and mode
   - Response injection

4. **I2C Bus**
   - Multi-device support
   - Address-based routing
   - Read/write operations

5. **Sensors**
   - Temperature, voltage, current
   - Realistic value ranges
   - Historical data tracking

### Failure Injection

The mock hardware can inject various failures for testing:

- Random operation failures
- Timeout simulation
- Communication errors
- Resource exhaustion

## CI Integration

### GitHub Actions Workflow

```yaml
name: HIL Tests
on: [push, pull_request]

jobs:
  hil-mock:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run HIL Tests (Mock Mode)
        run: .tests/hil/run_hil_tests.sh --mock
```

### Test Execution

```bash
# Run all HIL tests in mock mode (CI)
./.tests/hil/run_hil_tests.sh --mock

# Run specific test category
./.tests/hil/run_hil_tests.sh --mock --category unit

# Run with real hardware (lab)
./.tests/hil/run_hil_tests.sh --target raspberry_pi

# Verbose output
./.tests/hil/run_hil_tests.sh --mock --verbose
```

## Test Report Format

Tests generate JUnit XML reports for CI integration:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<testsuites>
  <testsuite name="hil_unit_gpio" tests="5" failures="0" time="0.523">
    <testcase name="test_gpio_write" time="0.102"/>
    <testcase name="test_gpio_read" time="0.098"/>
    ...
  </testsuite>
</testsuites>
```

## Hardware Targets

### Current Support

| Target | Status | Mock Available |
|--------|--------|----------------|
| Raspberry Pi 4/5 | Planned | Yes |
| NVIDIA Jetson | Planned | Yes |
| STM32 | Planned | Yes |
| ESP32 | Planned | Yes |

### Adding New Targets

1. Create target configuration in `hil_config.toml`
2. Implement target-specific mock if needed
3. Add target-specific tests
4. Document hardware requirements

## Metrics Collection

The HIL framework collects the following metrics:

- Operation latency (min/max/avg/p99)
- Success/failure rates
- Resource utilization
- Error type distribution

## Best Practices

1. **Isolation**: Each test should be independent
2. **Cleanup**: Reset hardware state between tests
3. **Timeouts**: Always set reasonable timeouts
4. **Logging**: Use structured logging for debugging
5. **Idempotency**: Tests should be repeatable

## Future Enhancements

1. Real-time performance monitoring
2. Hardware state visualization
3. Automated regression detection
4. Multi-target parallel testing
5. Hardware-in-the-loop with actual devices

## References

- [Clawdius Architecture](/.clawdius/specs/02_architecture/)
- [Test Vectors](/.clawdius/specs/01_research/test_vectors/)
- [CI/CD Pipeline](/.clawdius/specs/07_ci_cd/)

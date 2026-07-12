# orecchiette-sdr-pluto-rs

[![CI](https://github.com/isaacbentley/orecchiette-sdr-pluto-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/isaacbentley/orecchiette-sdr-pluto-rs/actions/workflows/ci.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/github/license/isaacbentley/orecchiette-sdr-pluto-rs.svg)](https://choosealicense.com/licenses/gpl-3.0/)

Analog Devices ADALM-Pluto (PlutoSDR) implementation of the [`SdrSource`](https://github.com/isaacbentley/orecchiette-sdr-source-rs) trait, leveraging the `industrial-io` crate which wraps the `libiio` C library.

## Overview

The PlutoSDR provides full-duplex operation and is extremely cost-effective. By using the official `libiio` library via `industrial-io`, this backend supports high-speed data capture over both USB and Gigabit Ethernet (via a USB-to-Ethernet adapter).

## Key Features

- **Dynamic Configuration**: Automatically discovers and configures the AD9361 PHY and RX channels based on your requested parameters.
- **Hardware Gain**: Full control over manual baseband gain (0–73 dB).
- **Adaptive Channel Hopping**: Modifies the `altvoltage0` (RX LO) IIO attribute on the fly, allowing extremely fast retuning without tearing down the stream.

## Installation

This crate relies on `libiio`, so it is restricted to macOS and Linux.

### System Prerequisites

You must install the `libiio` C library before building this crate.
- **Ubuntu/Debian**: `sudo apt install libiio-dev`
- **macOS**: `libiio` must be built from source as it is not available in Homebrew. Additionally, because the `libiio-sys` Rust bindings search for `iio.framework` in specific architecture-dependent directories, you must copy the built framework to the expected path:
  ```bash
  brew install cmake pkg-config libusb
  git clone --depth 1 --branch v0.25 https://github.com/analogdevicesinc/libiio.git
  cd libiio
  cmake -B build -DPYTHON_BINDINGS=OFF -DCSHARP_BINDINGS=OFF -DOSX_PACKAGE=OFF -DOSX_FRAMEWORK=ON
  cmake --build build

  # For Apple Silicon Macs (ARM64):
  mkdir -p /opt/homebrew/Frameworks
  cp -R build/iio.framework /opt/homebrew/Frameworks/

  # For Intel Macs (x86_64):
  mkdir -p /usr/local/Frameworks
  cp -R build/iio.framework /usr/local/Frameworks/
  ```

Add the following to your `Cargo.toml`:

```toml
[dependencies]
orecchiette-sdr-pluto-rs = "0.1.0"
orecchiette-sdr-source-rs = "0.1.0"
```

## Usage

```rust,no_run
use orecchiette_sdr_pluto_rs::PlutoSource;
use orecchiette_sdr_source_rs::{DwellAdvice, SdrSource, SourceConfig};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct NoSignalLog;
impl DwellAdvice for NoSignalLog {
    fn latest_signal_at(&self, _freq_key_khz: u64) -> Option<Instant> { None }
}

let advice: Arc<dyn DwellAdvice> = Arc::new(NoSignalLog);

let source = Box::new(PlutoSource {
    uri: "ip:192.168.2.1".to_string(), // Network connection to Pluto
    rx_gain_db: Some(70), // Max 73 dB
});

let config = SourceConfig {
    sample_rate_hz: 5_000_000.0, // Limit for standard USB / Network
    channels_hz: vec![5_800e6],
    dwell_min: Duration::from_secs(3600),
    dwell_max: Duration::from_secs(3600),
    dwell_extension: Duration::ZERO,
};

let handle = source.start(config, advice).unwrap();

for packet in handle.receiver.iter() {
    // Process complex IQ samples
}
```

## MSRV & Semver Policy

- **MSRV:** This crate does not maintain an explicit Minimum Supported Rust Version (MSRV) policy and tracks the latest `stable` compiler.
- **Semver:** This crate follows semantic versioning. While in `0.x.y`, breaking API changes will result in a minor version bump (e.g. `0.1.x` to `0.2.0`).

## Errors

`start()` validates `SourceConfig` — a non-empty `channels_hz` and a positive
`sample_rate_hz` — before opening the Pluto, so a bad config returns
`SdrError::BadConfig` without ever touching the device. Once streaming, a
channel that fails to refill its buffer 200 times in a row (~200ms) is
skipped in favor of the next channel; if every channel fails for 10
consecutive sweeps (~5+ seconds of an unresponsive device), the capture
thread gives up rather than retrying forever, and `SdrHandle::wait()`
returns once that happens.

## Testing & Contributing

4 unit tests cover the config validation and reliable-rate-ceiling check.
Please see [CONTRIBUTING.md](CONTRIBUTING.md) for detailed instructions on running the test suite and formatting your code before submitting a Pull Request.

## Documentation

- [Architecture & Design](DESIGN.md) — internal architecture and execution flow.

## License

This project is licensed under the GNU General Public License v3.0 or later (GPL-3.0-or-later) - see the [LICENSE](LICENSE) file for details.

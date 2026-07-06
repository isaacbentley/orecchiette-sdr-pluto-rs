# 📻 orecchiette-sdr-pluto-rs: ADALM-Pluto Interface

[![CI](https://github.com/isaacbentley/orecchiette-sdr-pluto-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/isaacbentley/orecchiette-sdr-pluto-rs/actions/workflows/ci.yml)
[![MSRV](https://img.shields.io/badge/rustc-1.85+-ab6000.svg)](https://blog.rust-lang.org/2025/02/20/Rust-1.85.0.html)

Analog Devices ADALM-Pluto (PlutoSDR) implementation of the [`SdrSource`](https://github.com/isaacbentley/orecchiette-sdr-source-rs) trait. This crate seamlessly integrates the PlutoSDR into the SDR detection applications orchestrator, leveraging the `industrial-io` crate which wraps the `libiio` C library.

## 🎯 **Why orecchiette-sdr-pluto-rs?**

The PlutoSDR provides full-duplex operation and is extremely cost-effective. By using the official `libiio` library via `industrial-io`, this backend supports high-speed data capture over both USB and Gigabit Ethernet (via a USB-to-Ethernet adapter).

## 🚀 **Features**

### **⚡ Advanced Device Control**
- **Dynamic Configuration**: Automatically discovers and configures the AD9361 PHY and RX channels based on your requested parameters.
- **Hardware Gain**: Full control over manual baseband gain (0–73 dB).
- **Adaptive Channel Hopping**: Modifies the `altvoltage0` (RX LO) IIO attribute on the fly, allowing extremely fast retuning without tearing down the stream.

## 📦 **Installation**

Because this crate relies on `libiio`, it is restricted to macOS and Linux by default and placed behind a feature flag in the main orchestrator to prevent build breaks on Windows.

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

Add to your `Cargo.toml`:
```toml
[dependencies]
orecchiette-sdr-pluto-rs = { git = "https://github.com/isaacbentley/orecchiette-sdr-pluto-rs.git", branch = "main" }
```

## 🔧 **Usage**

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

## 📚 **Documentation**

- [Architecture & Design](DESIGN.md) — internal architecture and execution flow.

## 📄 **License**

Licensed under the GNU General Public License v3.0 or later (GPL-3.0-or-later) - see the [LICENSE](../../LICENSE) file.

#![doc = include_str!("../README.md")]
//! ADALM-Pluto SDR source for SDR applications.

use crossbeam::channel;
use industrial_io::{Context, Direction};
use num_complex::Complex32;
use orecchiette_sdr_source_rs::{
    DwellAdvice, DwellController, IqPacket, SdrError, SdrHandle, SdrSource, SourceConfig,
    freq_key_khz,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use tracing::{info, warn};

pub const PLUTO_MAX_RELIABLE_RATE_HZ: f64 = 5_000_000.0;

struct SendWrapper<T>(T);

// SAFETY: libiio objects are created on the caller thread, moved once, and accessed
// solely on the single worker thread. They do not cross thread boundaries concurrently.
unsafe impl Send for SendWrapper<Context> {}
unsafe impl Send for SendWrapper<industrial_io::Device> {}
unsafe impl Send for SendWrapper<industrial_io::Channel> {}
unsafe impl Send for SendWrapper<industrial_io::Buffer> {}

impl<T> SendWrapper<T> {
    fn into_inner(self) -> T {
        self.0
    }
}

pub struct PlutoSource {
    pub uri: String,
    pub rx_gain_db: Option<i64>,
}

impl Default for PlutoSource {
    fn default() -> Self {
        Self {
            uri: "ip:192.168.2.1".to_string(),
            rx_gain_db: None, // Default to AGC
        }
    }
}

impl SdrSource for PlutoSource {
    fn start(
        self: Box<Self>,
        config: SourceConfig,
        advice: Arc<dyn DwellAdvice>,
    ) -> Result<SdrHandle, SdrError> {
        if config.channels_hz.is_empty() {
            return Err(SdrError::BadConfig("channels_hz must not be empty".into()));
        }

        if config.sample_rate_hz > PLUTO_MAX_RELIABLE_RATE_HZ {
            warn!(
                "[pluto] Requested {:.2} MSPS might cause overruns over USB/Network. Typical reliable limit is ~5 MSPS.",
                config.sample_rate_hz / 1e6
            );
        }

        info!("[pluto] Connecting to URI: {}", self.uri);
        let ctx = Context::from_uri(&self.uri).map_err(|e| {
            SdrError::NotFound(format!(
                "Failed to connect to Pluto at {}: {:?}",
                self.uri, e
            ))
        })?;

        let phy = ctx
            .find_device("ad9361-phy")
            .ok_or_else(|| SdrError::NotFound("Pluto ad9361-phy device not found".into()))?;

        let rx = ctx
            .find_device("cf-ad9361-lpc")
            .ok_or_else(|| SdrError::NotFound("Pluto cf-ad9361-lpc device not found".into()))?;

        let rx_phy = phy
            .find_channel("voltage0", Direction::Input)
            .ok_or_else(|| {
                SdrError::BadConfig("Failed to find voltage0 channel for config".into())
            })?;

        rx_phy
            .attr_write("sampling_frequency", config.sample_rate_hz as i64)
            .map_err(|e| SdrError::BadConfig(format!("Failed to set sample rate: {:?}", e)))?;

        let rf_bandwidth = (config.sample_rate_hz * 0.8) as i64;
        rx_phy
            .attr_write("rf_bandwidth", rf_bandwidth)
            .map_err(|e| SdrError::BadConfig(format!("Failed to set RF bandwidth: {:?}", e)))?;

        if let Some(gain) = self.rx_gain_db {
            rx_phy
                .attr_write("gain_control_mode", "manual")
                .map_err(|e| {
                    SdrError::BadConfig(format!("Failed to set manual gain mode: {:?}", e))
                })?;
            rx_phy.attr_write("hardwaregain", gain).map_err(|e| {
                SdrError::BadConfig(format!("Failed to set gain {}: {:?}", gain, e))
            })?;
        } else {
            rx_phy
                .attr_write("gain_control_mode", "slow_attack")
                .map_err(|e| {
                    SdrError::BadConfig(format!("Failed to set AGC (slow_attack) mode: {:?}", e))
                })?;
        }

        let rx_i = rx
            .find_channel("voltage0", Direction::Input)
            .ok_or_else(|| SdrError::BadConfig("Failed to find RX voltage0 channel".into()))?;
        let rx_q = rx
            .find_channel("voltage1", Direction::Input)
            .ok_or_else(|| SdrError::BadConfig("Failed to find RX voltage1 channel".into()))?;

        rx_i.enable();
        rx_q.enable();

        let buffer = rx
            .create_buffer(32768, false)
            .map_err(|e| SdrError::BadConfig(format!("Failed to create IIO buffer: {:?}", e)))?;

        let (pool_tx, pool_rx) = crossbeam::channel::bounded::<Vec<Complex32>>(256);
        for _ in 0..256 {
            let _ = pool_tx.send(Vec::with_capacity(32768));
        }

        let rx_lo = phy
            .find_channel("altvoltage0", Direction::Output)
            .ok_or_else(|| {
                SdrError::BadConfig("Failed to find altvoltage0 channel for RX LO".into())
            })?;

        let dwell_controller = DwellController {
            min: config.dwell_min,
            max: config.dwell_max,
            extension: config.dwell_extension,
        };
        let channels_hz = config.channels_hz.clone();
        let num_channels = channels_hz.len();

        let (tx, receiver) = channel::bounded::<IqPacket>(1024);
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_thread = stop_flag.clone();
        let advice_thread = advice;
        let sample_rate_f32 = config.sample_rate_hz as f32;

        let w_ctx = SendWrapper(ctx);
        let w_phy = SendWrapper(phy);
        let w_rx = SendWrapper(rx);
        let w_buffer = SendWrapper(buffer);
        let w_rx_lo = SendWrapper(rx_lo);
        let w_rx_i = SendWrapper(rx_i);
        let w_rx_q = SendWrapper(rx_q);

        #[allow(clippy::redundant_closure_call)]
        let capture_thread = thread::spawn(move || {
            if let Err(e) = (move || -> Result<(), anyhow::Error> {
                let _ctx = w_ctx.into_inner();
                let _phy = w_phy.into_inner();
                let _rx = w_rx.into_inner();
                let mut buffer = w_buffer.into_inner();
                let rx_lo = w_rx_lo.into_inner();
                let rx_i = w_rx_i.into_inner();
                let rx_q = w_rx_q.into_inner();

                let mut channel_idx = 0usize;
                let mut last_report = Instant::now();
                let mut channel_switches = 0u64;
                let mut last_freq_hz: Option<f64> = None;

                // Expected time to fill one 32k buffer
                let expected_duration = Duration::from_secs_f64(32768.0 / sample_rate_f32 as f64);
                // libiio typically uses 4 kernel buffers. If we lag by more than 3 buffers' worth of time,
                // the kernel ring has overflowed and dropped samples.
                let max_lag = expected_duration * 3;

                'outer: loop {
                    if stop_flag_thread.load(Ordering::SeqCst) {
                        break;
                    }

                    let current_freq_hz = channels_hz[channel_idx];
                    let freq_key = freq_key_khz(current_freq_hz);

                    if last_freq_hz != Some(current_freq_hz) {
                        if let Err(e) = rx_lo.attr_write("frequency", current_freq_hz as i64) {
                            warn!("[pluto] Failed to retune to {}: {:?}", current_freq_hz, e);
                            channel_idx = (channel_idx + 1) % num_channels;
                            channel_switches += 1;
                            continue;
                        }
                        last_freq_hz = Some(current_freq_hz);
                    }

                    if num_channels > 1 {
                        thread::sleep(Duration::from_millis(5));
                    }

                    let dwell_start = Instant::now();
                    let mut first_refill = true;
                    let mut last_refill_time = Instant::now();

                    loop {
                        if stop_flag_thread.load(Ordering::SeqCst) {
                            break 'outer;
                        }

                        let latest_signal = advice_thread.latest_signal_at(freq_key);
                        let deadline = dwell_controller.deadline(dwell_start, latest_signal);
                        if Instant::now() >= deadline {
                            break;
                        }

                        if let Err(e) = buffer.refill() {
                            warn!("[pluto] Buffer refill error: {:?}", e);
                            thread::sleep(Duration::from_millis(1));
                            last_refill_time = Instant::now();
                            continue;
                        }

                        let now = Instant::now();
                        let overrun = if first_refill {
                            first_refill = false;
                            false
                        } else {
                            now.duration_since(last_refill_time) > max_lag
                        };
                        last_refill_time = now;

                        let i_data = match rx_i.read::<i16>(&buffer) {
                            Ok(data) => data,
                            Err(e) => {
                                warn!("[pluto] Failed to read I channel: {:?}", e);
                                continue;
                            }
                        };
                        let q_data = match rx_q.read::<i16>(&buffer) {
                            Ok(data) => data,
                            Err(e) => {
                                warn!("[pluto] Failed to read Q channel: {:?}", e);
                                continue;
                            }
                        };

                        if i_data.len() != q_data.len() {
                            warn!("[pluto] I/Q length mismatch");
                            continue;
                        }

                        let mut samples = pool_rx
                            .try_recv()
                            .unwrap_or_else(|_| Vec::with_capacity(i_data.len()));
                        samples.clear();
                        for (i, q) in i_data.into_iter().zip(q_data) {
                            samples.push(Complex32::new((i as f32) / 2048.0, (q as f32) / 2048.0));
                        }

                        let pkt = IqPacket {
                            samples: orecchiette_sdr_source_rs::PooledIqBuffer::new_pooled(
                                samples,
                                pool_tx.clone(),
                            ),
                            center_frequency_hz: current_freq_hz,
                            sample_rate_hz: sample_rate_f32,
                            overrun,
                        };

                        if tx.send(pkt).is_err() {
                            break 'outer;
                        }

                        if last_report.elapsed() >= Duration::from_secs(60) {
                            let rate =
                                channel_switches as f32 / last_report.elapsed().as_secs_f32();
                            info!(
                                "[pluto] Scanning speed: {:.1} ch/s | Pool size: {} channels",
                                rate, num_channels
                            );
                            channel_switches = 0;
                            last_report = Instant::now();
                        }
                    }

                    channel_idx = (channel_idx + 1) % num_channels;
                    channel_switches += 1;
                }
                Ok(())
            })() {
                tracing::error!("[pluto] Capture thread failed: {:?}", e);
            }
        });

        let stop_flag_for_stop = stop_flag.clone();
        let stop = Box::new(move || {
            stop_flag_for_stop.store(true, Ordering::SeqCst);
        });
        let wait = Box::new(move || {
            if let Err(e) = capture_thread.join() {
                tracing::error!("[pluto] capture thread join failed: {:?}", e);
            }
        });

        Ok(SdrHandle {
            receiver,
            stop,
            wait,
        })
    }
}

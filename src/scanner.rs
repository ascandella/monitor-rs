use std::collections::HashMap;

use anyhow::Context as _;

use log::{debug, error, info};
use tokio::process::Command;
use tokio::sync::broadcast;

use crate::{
    config::{BleDevice, ScanConfig},
    messages::{DeviceAnnouncement, StateAnnouncement},
};

pub struct Scanner {
    rx: broadcast::Receiver<StateAnnouncement>,
    device_seen_debounce: std::time::Duration,
    device_trigger_debounce: std::time::Duration,
    interscan_delay: std::time::Duration,
    announce_rx: broadcast::Sender<DeviceAnnouncement>,
    device_map: HashMap<String, DeviceState>,
}

#[derive(Debug)]
struct DeviceState {
    mac_address: String,
    seen: DeviceSeen,
}

#[derive(Debug)]
enum DeviceSeen {
    Seen(std::time::SystemTime),
    NotSeen,
}

impl Scanner {
    pub fn new(
        cfg: &ScanConfig,
        rx: broadcast::Receiver<StateAnnouncement>,
        announce_rx: broadcast::Sender<DeviceAnnouncement>,
        devices: &[BleDevice],
    ) -> Self {
        let device_map = devices
            .iter()
            .map(|device| {
                (
                    device.name.clone(),
                    DeviceState {
                        mac_address: device.address.to_string(),
                        seen: DeviceSeen::NotSeen,
                    },
                )
            })
            .collect::<HashMap<_, _>>();

        Scanner {
            rx,
            announce_rx,
            device_seen_debounce: std::time::Duration::from_secs(
                cfg.device_seen_debounce_seconds.unwrap_or(60),
            ),
            device_trigger_debounce: std::time::Duration::from_secs(
                cfg.device_trigger_debounce_seconds.unwrap_or(120),
            ),
            interscan_delay: std::time::Duration::from_secs(
                cfg.interscan_delay_seconds.unwrap_or(5),
            ),
            device_map,
        }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        debug!("Start scan loop {:?}", self.device_map);
        let mut last_trigger: Option<std::time::SystemTime> = None;
        loop {
            match self.rx.recv().await {
                // Handle incoming MQTT messages (e.g. arrival scan requests)
                Ok(msg) => match msg {
                    StateAnnouncement::ScanArrive => {
                        info!("Received arrival scan request");
                        last_trigger = Some(std::time::SystemTime::now());
                        self.scan_arrival()
                            .await
                            .context("Failed to scan arrivals")?;
                    }
                    StateAnnouncement::ScanDepart => {
                        info!("Received departure request");
                        last_trigger = Some(std::time::SystemTime::now());
                        self.scan_departure()
                            .await
                            .context("Failed to scan departure")?;
                    }
                    StateAnnouncement::DeviceTrigger => {
                        let should_scan_devices = match last_trigger.map(|t| t.elapsed()) {
                            Some(Ok(duration)) => {
                                if duration > self.device_trigger_debounce {
                                    debug!("Device trigger received after {:?}", duration);
                                    true
                                } else {
                                    debug!("Device trigger received too soon, ignoring");
                                    false
                                }
                            }
                            Some(Err(err)) => {
                                error!(
                                    "Unable to calculate duration since last trigger: {:?}",
                                    err
                                );
                                true
                            }
                            None => {
                                debug!("Device trigger received, no previous trigger time");
                                true
                            }
                        };
                        if should_scan_devices {
                            info!("Triggering scan due to new device matching manufacturer filter");
                            last_trigger = Some(std::time::SystemTime::now());
                            self.scan_arrival()
                                .await
                                .context("Failed to scan for device trigger")?;
                        }
                    }
                },
                Err(broadcast::error::RecvError::Closed) => {
                    debug!("Receiver closed");
                    break;
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    debug!("Receiver lagged");
                }
            }
        }
        Ok(())
    }

    async fn scan_arrival(&mut self) -> anyhow::Result<()> {
        let mut scan_count = 0;
        for (name, device_info) in self.device_map.iter_mut() {
            let now = std::time::SystemTime::now();
            let should_scan = match device_info.seen {
                DeviceSeen::Seen(at) => match now.duration_since(at) {
                    Ok(duration) => {
                        if duration > self.device_seen_debounce {
                            debug!("Device {} hasn't been seen in {:?}", name, duration);
                            true
                        } else {
                            debug!("Device {} is seen recently ({:?}), not scanning", name, at);
                            false
                        }
                    }
                    Err(err) => {
                        error!(
                            "Unable to calculate duration since last seen: {}, {:?}",
                            name, err
                        );
                        true
                    }
                },
                DeviceSeen::NotSeen => {
                    debug!(
                        "Device {} currently marked as absent, is candidate for arrival scan",
                        name
                    );
                    true
                }
            };

            if should_scan {
                if scan_count > 0 {
                    tokio::time::sleep(self.interscan_delay).await;
                }
                scan_device(name, device_info, &self.announce_rx).await?;
                scan_count += 1;
            }
        }

        Ok(())
    }

    async fn scan_departure(&mut self) -> anyhow::Result<()> {
        for (scan_count, (name, device_info)) in self.device_map.iter_mut().enumerate() {
            if scan_count > 0 {
                tokio::time::sleep(self.interscan_delay).await;
            }
            scan_device(name, device_info, &self.announce_rx).await?;
        }

        Ok(())
    }
}

async fn scan_device(
    name: &str,
    device_info: &mut DeviceState,
    announce_rx: &broadcast::Sender<DeviceAnnouncement>,
) -> anyhow::Result<()> {
    let now = std::time::SystemTime::now();
    if is_device_present(device_info).await? {
        device_info.seen = DeviceSeen::Seen(now);
        announce_device(
            announce_rx,
            name,
            &device_info.mac_address,
            crate::messages::DevicePresence::Present(100),
        )
    } else {
        debug!("Device {} is not present", name);
        device_info.seen = DeviceSeen::NotSeen;
        announce_device(
            announce_rx,
            name,
            &device_info.mac_address,
            crate::messages::DevicePresence::Absent,
        )
    }
}

fn announce_device(
    announce_rx: &broadcast::Sender<DeviceAnnouncement>,
    name: &str,
    mac_address: &str,
    presence: crate::messages::DevicePresence,
) -> anyhow::Result<()> {
    announce_rx
        .send(DeviceAnnouncement {
            name: name.to_string(),
            mac_address: mac_address.to_string(),
            presence,
        })
        .context("Failed to send device announcement")?;

    Ok(())
}

/// Shell out to `hcitool name <MAC>` like the Bash version of this utility does.
/// Theoretically this is something that could be done in Rust, but `btleplug` only supports direct
/// connecting via MAC address on Android, not Windows/Linux/macOS. That means this
/// function only works on Linux, since `hcitool` is a `bluez` utility.
async fn is_device_present(state: &DeviceState) -> anyhow::Result<bool> {
    let output = Command::new("hcitool")
        .arg("name")
        .arg(&state.mac_address)
        .output()
        .await?;

    if output.status.success() {
        let output_str = String::from_utf8_lossy(&output.stdout);
        if output_str.is_empty() {
            debug!(
                "Device {} is not present: empty reply from hcitool",
                state.mac_address
            );
            Ok(false)
        } else {
            debug!(
                "Device {} is present: hcitool returned '{}'",
                state.mac_address,
                output_str.trim()
            );
            Ok(true)
        }
    } else {
        Err(anyhow::anyhow!(
            "Command exited non-zero {:?}",
            output.stderr
        ))
    }
}

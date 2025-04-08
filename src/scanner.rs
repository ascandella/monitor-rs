use std::collections::HashMap;

use log::{debug, info};
use tokio::sync::broadcast;

use crate::{config::BleDevice, mqtt::MqttAnnouncement};

pub struct Scanner {
    rx: broadcast::Receiver<MqttAnnouncement>,
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
    pub fn new(rx: broadcast::Receiver<MqttAnnouncement>, devices: &[BleDevice]) -> Self {
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
        Scanner { rx, device_map }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Start scan loop {:?}", self.device_map);
        loop {
            match self.rx.recv().await {
                // Handle incoming MQTT messages (e.g. arrival scan requests)
                Ok(msg) => match msg {
                    MqttAnnouncement::ScanArrive => {
                        info!("Received arrival scan request");
                        unimplemented!("Start arrival scan");
                    }
                    MqttAnnouncement::ScanDepart => {
                        info!("Received departure request");
                        unimplemented!("Need to handle this");
                    }
                    MqttAnnouncement::DeviceTrigger => {
                        info!("Received device trigger request");
                        unimplemented!("Need to handle this");
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
}

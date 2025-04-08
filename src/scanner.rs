use std::collections::HashMap;

use log::{debug, info};
use tokio::sync::broadcast;

use crate::{
    config::BleDevice,
    messages::{DeviceAnnouncement, StateAnnouncement},
};

pub struct Scanner {
    rx: broadcast::Receiver<StateAnnouncement>,
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
            device_map,
        }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Start scan loop {:?}", self.device_map);
        loop {
            match self.rx.recv().await {
                // Handle incoming MQTT messages (e.g. arrival scan requests)
                Ok(msg) => match msg {
                    StateAnnouncement::ScanArrive => {
                        info!("Received arrival scan request");
                        self.scan_arrival().await;
                    }
                    StateAnnouncement::ScanDepart => {
                        info!("Received departure request");
                        self.scan_departure().await;
                    }
                    StateAnnouncement::DeviceTrigger => {
                        debug!("Received device trigger request");
                        self.scan_arrival().await;
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

    async fn scan_arrival(&mut self) {
        let (name, device_info) = self.device_map.iter_mut().next().unwrap();
        device_info.seen = DeviceSeen::Seen(std::time::SystemTime::now());
        // TODO
        self.announce_rx
            .send(DeviceAnnouncement {
                name: name.to_string(),
                mac_address: device_info.mac_address.clone(),
                presence: crate::messages::DevicePresence::Present(100),
            })
            .unwrap();
        // Loop every every device we haven't seen recently, trigger a name
        // request
        // unimplemented!("Start arrival scan");
    }

    async fn scan_departure(&mut self) {
        let (name, device_info) = self.device_map.iter_mut().next().unwrap();
        device_info.seen = DeviceSeen::NotSeen;
        // TODO
        self.announce_rx
            .send(DeviceAnnouncement {
                name: name.to_string(),
                mac_address: device_info.mac_address.clone(),
                presence: crate::messages::DevicePresence::Absent,
            })
            .unwrap();
        // unimplemented!("Start departure scan");
    }
}

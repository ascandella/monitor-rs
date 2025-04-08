use log::{debug, info};
use tokio::sync::broadcast;

use crate::mqtt::MqttAnnouncement;

pub struct Scanner {
    rx: broadcast::Receiver<MqttAnnouncement>,
}

impl Scanner {
    pub fn new(rx: broadcast::Receiver<MqttAnnouncement>) -> Self {
        Scanner { rx }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            match self.rx.recv().await {
                // Handle incoming MQTT messages (e.g. arrival scan requests)
                Ok(msg) => match msg {
                    MqttAnnouncement::ScanArrive => {
                        info!("Received arrival scan request");
                        todo!("Start arrival scan");
                    }
                    MqttAnnouncement::ScanDepart => {
                        info!("Received departure request");
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

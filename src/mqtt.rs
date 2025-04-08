use std::time::Duration;

use log::{debug, error};
use rumqttc::{MqttOptions, QoS};
use tokio::sync::broadcast;

use crate::{config, messages::StateAnnouncement};

pub struct MqttClient {
    client: rumqttc::AsyncClient,
    topic_path: String,
}

impl MqttClient {
    pub fn new(config: &config::MqttConfig) -> (Self, rumqttc::EventLoop) {
        let mut mqttoptions = MqttOptions::new(
            config
                .publisher_id
                .as_ref()
                .unwrap_or(&"monitor-rs".to_string())
                .to_string(),
            config.host.clone(),
            config.port.unwrap_or(1883),
        );

        mqttoptions.set_keep_alive(Duration::from_secs(config.keep_alive_seconds.unwrap_or(5)));

        if let (Some(username), Some(password)) =
            (config.username.as_ref(), config.password.as_ref())
        {
            mqttoptions.set_credentials(username.clone(), password.clone());
        }

        let (client, eventloop) = rumqttc::AsyncClient::new(mqttoptions, 10);

        (
            MqttClient {
                client,
                topic_path: config.topic_path.clone().unwrap_or("monitor".to_string()),
            },
            eventloop,
        )
    }

    pub async fn subscribe(&self) -> Result<(), rumqttc::ClientError> {
        self.client
            .subscribe(format!("{}/scan/arrive", self.topic_path), QoS::AtMostOnce)
            .await?;

        Ok(())
    }

    pub async fn event_loop(
        eventloop: &mut rumqttc::EventLoop,
        tx: broadcast::Sender<StateAnnouncement>,
    ) {
        loop {
            match eventloop.poll().await {
                Ok(notification) => match notification {
                    rumqttc::Event::Incoming(rumqttc::Packet::Publish(p)) => {
                        let payload = p.payload;
                        // b"{\"id\":\"<mac address>\",\"confidence\":\"0\",\"name\":\"<name>\",\"manufacturer\":\"Apple Inc\",\"type\":\"KNOWN_MAC\",\"retained\":\"false\",\"timestamp\":\"2025-04-06T13:23:39-0700\",\"version\":\"0.2.200\"}"
                        debug!("Received MQTT message on topic {}: {:?}", p.topic, payload);

                        let message = match p.topic {
                            t if t.ends_with("/arrive") => StateAnnouncement::ScanArrive,
                            _ => StateAnnouncement::ScanDepart,
                        };

                        if let Err(err) = tx.send(message) {
                            error!("Error announcing scan: {:?}", err);
                        }
                    }
                    rumqttc::Event::Incoming(rumqttc::Packet::SubAck(_)) => {
                        debug!("Subscription acknowledged");
                    }
                    _ => {}
                },
                Err(e) => {
                    error!("Error polling MQTT event loop: {:?}", e);
                }
            }
        }
    }

    pub async fn disconnect(&self) -> Result<(), rumqttc::ClientError> {
        debug!("Disconnecting MQTT client");
        self.client.disconnect().await
    }
}

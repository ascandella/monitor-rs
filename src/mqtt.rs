use std::time::Duration;

use log::{debug, error, info};
use rumqttc::{MqttOptions, QoS, SubscribeFilter};
use serde::Serialize;
use tokio::sync::broadcast;

use crate::{config, messages::StateAnnouncement};

#[derive(Debug, Clone)]
pub struct MqttClient {
    client: rumqttc::AsyncClient,
    publisher_id: String,
    topic_path: String,
}

#[derive(Debug, Serialize)]
struct DeviceMqttMessage {
    name: String,
    #[serde(rename = "id")]
    mac_address: String,
    confidence: u8,
    retained: bool,
}

impl MqttClient {
    pub fn new(config: &config::MqttConfig) -> (Self, rumqttc::EventLoop) {
        let publisher_id = config
            .publisher_id
            .as_ref()
            .unwrap_or(&"monitor-rs".to_string())
            .to_string();

        let mut mqttoptions = MqttOptions::new(
            publisher_id.clone(),
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
                publisher_id,
                topic_path: config.topic_path.clone().unwrap_or("monitor".to_string()),
            },
            eventloop,
        )
    }

    pub async fn subscribe(&self) -> Result<(), rumqttc::ClientError> {
        self.client
            .subscribe_many(vec![
                SubscribeFilter::new(format!("{}/scan/arrive", self.topic_path), QoS::AtMostOnce),
                SubscribeFilter::new(format!("{}/scan/depart", self.topic_path), QoS::AtMostOnce),
            ])
            .await?;

        Ok(())
    }

    pub async fn event_loop(
        &self,
        eventloop: &mut rumqttc::EventLoop,
        tx: broadcast::Sender<StateAnnouncement>,
    ) {
        loop {
            match eventloop.poll().await {
                Ok(notification) => match notification {
                    rumqttc::Event::Incoming(rumqttc::Packet::Publish(p)) => {
                        let payload = p.payload;
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
                    rumqttc::Event::Incoming(rumqttc::Packet::ConnAck(_)) => {
                        debug!("Connection acknowledged");
                        if let Err(err) = self.subscribe().await {
                            error!("Error subscribing to MQTT topics: {:?}", err);
                        }
                    }
                    _ => {}
                },
                Err(e) => {
                    error!("Error polling MQTT event loop: {:?}", e);
                }
            }
        }
    }

    pub async fn announce_device(
        &self,
        name: &str,
        mac_address: String,
        confidence: u8,
    ) -> Result<(), rumqttc::ClientError> {
        info!(
            "Announcing device {} (confidence: {}) on MQTT",
            name, confidence
        );
        // TODO: Implement device tracker (`home` / `not_home`)
        // b"{\"id\":\"<mac address>\",\"confidence\":\"0\",\"name\":\"<name>\",\"manufacturer\":\"Apple Inc\",\"type\":\"KNOWN_MAC\",\"retained\":\"false\",\"timestamp\":\"2025-04-06T13:23:39-0700\",\"version\":\"0.2.200\"}"
        let message = DeviceMqttMessage {
            name: name.to_string(),
            mac_address,
            confidence,
            retained: false,
        };
        let channel_name = sanitize_name(name);
        self.client
            .publish(
                format!("{}/{}/{}", self.topic_path, self.publisher_id, channel_name),
                QoS::AtMostOnce,
                false,
                serde_json::to_string(&message).unwrap(),
            )
            .await
    }

    pub async fn disconnect(&self) -> Result<(), rumqttc::ClientError> {
        debug!("Disconnecting MQTT client");
        self.client.disconnect().await
    }
}

fn sanitize_name(name: &str) -> String {
    // Remove any non-alphanumeric characters and replace spaces with underscores
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_sanitize_name() {
        let name = "Test's Device 123";
        let sanitized = super::sanitize_name(name);
        assert_eq!(sanitized, "test_s_device_123");
    }
}

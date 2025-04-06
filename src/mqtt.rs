use std::time::Duration;

use rumqttc::{MqttOptions, QoS};

use crate::config;

pub struct MqttClient {
    client: rumqttc::AsyncClient,
    eventloop: rumqttc::EventLoop,
    topic_path: String,
}

impl MqttClient {
    pub fn new(config: &config::MqttConfig) -> Self {
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

        MqttClient {
            client,
            eventloop,
            topic_path: config.topic_path.clone().unwrap_or("monitor".to_string()),
        }
    }

    pub async fn subscribe(&self) -> Result<(), rumqttc::ClientError> {
        self.client
            .subscribe(format!("{}/scan/arrive", self.topic_path), QoS::AtMostOnce)
            .await
    }

    pub async fn disconnect(&self) -> Result<(), rumqttc::ClientError> {
        // Disconnect from the MQTT broker
        self.client.disconnect().await
    }
}

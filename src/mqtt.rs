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
            .subscribe(
                format!("{}/garage/robin_s_iphone", self.topic_path),
                QoS::AtMostOnce,
            )
            .await?;

        Ok(())
    }

    pub async fn event_loop(&mut self) {
        loop {
            match self.eventloop.poll().await {
                Ok(notification) => match notification {
                    rumqttc::Event::Incoming(rumqttc::Packet::Publish(p)) => {
                        let payload = p.payload;
                        // b"{\"id\":\"<mac address>\",\"confidence\":\"0\",\"name\":\"<name>\",\"manufacturer\":\"Apple Inc\",\"type\":\"KNOWN_MAC\",\"retained\":\"false\",\"timestamp\":\"2025-04-06T13:23:39-0700\",\"version\":\"0.2.200\"}"
                        println!("Received message: {:?}", payload);
                    }
                    rumqttc::Event::Incoming(rumqttc::Packet::SubAck(_)) => {
                        println!("Subscription acknowledged");
                    }
                    _ => {}
                },
                Err(e) => {
                    eprintln!("Error: {:?}", e);
                }
            }
        }
    }

    pub async fn disconnect(&self) -> Result<(), rumqttc::ClientError> {
        // Disconnect from the MQTT broker
        self.client.disconnect().await
    }
}

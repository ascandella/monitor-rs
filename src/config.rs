use serde_derive::Deserialize;

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct AppConfig {
    mqtt: MQTTConfig,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct MQTTConfig {
    host: String,
    port: Option<u16>,
    username: String,
    password: String,
    publisher_id: String,
    topic_path: Option<String>,
}

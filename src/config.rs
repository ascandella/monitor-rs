use serde_derive::Deserialize;

#[derive(Deserialize, Debug)]
pub struct AppConfig {
    pub mqtt: MqttConfig,
    pub devices: Option<Vec<BtleDevice>>,
}

#[derive(Deserialize, Debug)]
pub struct MqttConfig {
    pub host: String,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub publisher_id: Option<String>,
    pub topic_path: Option<String>,
    pub keep_alive_seconds: Option<u64>,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct BtleDevice {
    pub address: String,
    pub name: String,
}

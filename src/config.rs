use mac_address::MacAddress;
use serde_derive::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub mqtt: MqttConfig,
    pub devices: Option<Vec<BleDevice>>,
    pub scan: Option<ScanConfig>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct MqttConfig {
    pub host: String,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub publisher_id: Option<String>,
    pub topic_path: Option<String>,
    pub keep_alive_seconds: Option<u64>,
}

#[derive(Deserialize, Debug, Clone)]
pub enum Manufacturer {
    Apple,
    Google,
}

impl Manufacturer {
    /// https://bitbucket.org/bluetooth-SIG/public/src/main/assigned_numbers/company_identifiers/company_identifiers.yaml
    pub fn company_ids(&self) -> Vec<u16> {
        match self {
            Manufacturer::Apple => vec![0x004C],
            Manufacturer::Google => vec![0x018E, 0x00E0],
        }
    }
}

#[allow(dead_code)]
#[derive(Deserialize, Debug, Clone)]
pub struct BleDevice {
    pub address: MacAddress,
    pub name: String,
    pub manufacturer: Option<Manufacturer>,
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct ScanConfig {
    pub device_seen_debounce_seconds: Option<u64>,
    pub device_trigger_debounce_seconds: Option<u64>,
    pub interscan_delay_seconds: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config() {
        let config_str = r#"
            [mqtt]
            host = "localhost"
            port = 1883
            username = "user"
            password = "pass"

            [scan]
            device_seen_debounce_seconds = 60
            device_trigger_debounce_seconds = 60
            interscan_delay_seconds = 5
        "#;
        let config: AppConfig = toml::de::from_str(&config_str).unwrap();
        assert!(config.mqtt.host == "localhost");
        assert!(config.scan.is_some());
        assert!(config.scan.map(|s| s.device_seen_debounce_seconds).unwrap() == Some(60));
    }
}

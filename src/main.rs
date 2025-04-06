use btleplug::api::Manager as _;
use btleplug::platform::Manager;
use std::error::Error;
use std::fs::File;
use std::io::Read as _;

mod config;
mod manager;
mod mqtt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // TODO: CLI argument to specify config file
    let mut file = File::open("config.toml")?;
    let mut config_contents = String::new();
    file.read_to_string(&mut config_contents)?;

    let config: config::AppConfig = toml::de::from_str(&config_contents)?;

    println!("Devices: {:?}", config.devices);

    let (mqtt_client, eventloop) = mqtt::MqttClient::new(&config.mqtt);
    mqtt_client.subscribe().await?;

    let bt_manager = Manager::new().await?;

    // get the first bluetooth adapter
    let adapters = bt_manager.adapters().await?;
    let central = adapters.into_iter().next().unwrap();

    let core = manager::Manager::new(central, mqtt_client, eventloop);
    core.run_loop().await?;

    Ok(())
}

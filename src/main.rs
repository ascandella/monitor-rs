use btleplug::api::Manager as _;
use btleplug::platform::Manager;
use clap::{Parser, arg};
use log::{LevelFilter, debug, info};
use std::error::Error;
use std::fs::File;
use std::io::Read as _;

mod config;
mod manager;
mod messages;
mod mqtt;
mod scanner;

#[derive(Parser, Debug)]
struct Args {
    /// Path to the config file
    #[arg(short, long, default_value = "config.toml")]
    config: String,

    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let default_level = if args.verbose {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };

    pretty_env_logger::formatted_builder()
        .filter_module("monitor_rs", default_level)
        .parse_default_env()
        .init();

    let mut file = File::open(args.config)?;
    let mut config_contents = String::new();
    file.read_to_string(&mut config_contents)?;

    let config: config::AppConfig = toml::de::from_str(&config_contents)?;

    debug!("Configured to look for devices: {:?}", config.devices);

    let (mqtt_client, eventloop) = mqtt::MqttClient::new(&config.mqtt);

    let bt_manager = Manager::new().await?;

    // get the first bluetooth adapter
    let adapters = bt_manager.adapters().await?;
    let central = adapters
        .into_iter()
        .next()
        .ok_or("No Bluetooth adapter found")?;

    info!("Devices initialized, starting event loop");

    let core = manager::Manager::new(&config, central, mqtt_client, eventloop);
    core.run_loop().await?;

    Ok(())
}

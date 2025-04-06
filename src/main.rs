use btleplug::api::{Central, CentralEvent, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::Manager;
use futures::stream::StreamExt;
use std::error::Error;
use std::fs::File;
use std::io::Read as _;

mod config;
mod mqtt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // TODO: CLI argument to specify config file
    let mut file = File::open("config.toml")?;
    let mut config_contents = String::new();
    file.read_to_string(&mut config_contents)?;

    let config: config::AppConfig = toml::de::from_str(&config_contents)?;

    let mut mqtt_client = mqtt::MqttClient::new(&config.mqtt);
    mqtt_client.subscribe().await?;

    let manager = Manager::new().await?;

    // get the first bluetooth adapter
    let adapters = manager.adapters().await?;
    let central = adapters.into_iter().next().unwrap();

    // Each adapter has an event stream, we fetch via events(),
    // simplifying the type, this will return what is essentially a
    // Future<Result<Stream<Item=CentralEvent>>>.
    let mut events = central.events().await?;

    // start scanning for devices
    central.start_scan(ScanFilter::default()).await?;

    tokio::task::spawn(async move {
        mqtt_client.event_loop().await;
    });

    // Print based on whatever the event receiver outputs. Note that the event
    // receiver blocks, so in a real program, this should be run in its own
    // thread (not task, as this library does not yet use async channels).
    while let Some(event) = events.next().await {
        match event {
            CentralEvent::DeviceDiscovered(id) => {
                let peripheral = central.peripheral(&id).await?;
                let properties = peripheral.properties().await?;
                let name = properties
                    .and_then(|p| p.local_name)
                    .map(|local_name| format!("Name: {local_name}"))
                    .unwrap_or_default();
                println!("DeviceDiscovered: {:?} {}", id, name);
            }
            CentralEvent::StateUpdate(state) => {
                println!("AdapterStatusUpdate {:?}", state);
            }
            CentralEvent::DeviceDisconnected(id) => {
                println!("DeviceDisconnected: {:?}", id);
            }
            _ => {}
        }
    }

    Ok(())
}

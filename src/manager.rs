use btleplug::api::{Central as _, CentralEvent, Peripheral as _, ScanFilter};
use futures::{StreamExt as _, executor::block_on};
use tokio::sync::broadcast;

pub struct Manager {
    adapter: btleplug::platform::Adapter,
    mqtt_client: crate::mqtt::MqttClient,
    mqtt_event_loop: rumqttc::EventLoop,
}

async fn handle_btle_events(
    adapter: &btleplug::platform::Adapter,
    mut rx: broadcast::Receiver<crate::mqtt::MqttAnnouncement>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut events = adapter.events().await?;

    let mut event_stream_closed = false;

    loop {
        if event_stream_closed {
            break;
        }
        tokio::select! {
            // Handle incoming MQTT messages (e.g. arrival scan requests)
            Ok(msg) = rx.recv() => {
                match msg {
                    crate::mqtt::MqttAnnouncement::ScanArrive => {
                        println!("Received scan request");
                        adapter.start_scan(ScanFilter::default()).await?;
                    }
                }
            }
            event = events.next() => {
                match event {
                    Some(CentralEvent::DeviceDiscovered(id)) => {
                        let peripheral = adapter.peripheral(&id).await?;
                        let properties = peripheral.properties().await?;
                        let name = properties
                            .and_then(|p| p.local_name)
                            .map(|local_name| format!("Name: {local_name}"))
                            .unwrap_or_default();
                        println!("DeviceDiscovered: {:?} {}", id, name);
                    }
                        Some(_) => {}
                        None => {
                            println!("No more events");
                            event_stream_closed = true;
                        }
                }
            }
            else => {}
        }
    }
    Ok(())
}

impl Manager {
    pub fn new(
        adapter: btleplug::platform::Adapter,
        mqtt_client: crate::mqtt::MqttClient,
        mqtt_event_loop: rumqttc::EventLoop,
    ) -> Self {
        Manager {
            adapter,
            mqtt_client,
            mqtt_event_loop,
        }
    }

    pub async fn run_loop(mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.adapter.start_scan(ScanFilter::default()).await?;

        let (tx, rx) = broadcast::channel(10);

        // Handle incoming MQTT messages (e.g. arrival scan requests)
        tokio::task::spawn(async move {
            // TODO: Need to pass ability to trigger a BTLE scan to the event loop
            crate::mqtt::MqttClient::event_loop(&mut self.mqtt_event_loop, tx).await;
        });

        // Run on a separate thread as these currently block
        let btle_handle = std::thread::spawn(move || {
            // TODO: Need to pass the ability to publish MQTT messages to this function
            if let Err(err) = block_on(handle_btle_events(&self.adapter, rx)) {
                eprintln!("Error handling BTLE events: {:?}", err);
            }
            println!("Done handling BTLE events")
        });

        if let Err(err) = btle_handle.join() {
            eprintln!("Error handling btle events: {:?}", err);
        }
        println!("Exiting manager event loop");

        self.mqtt_client.disconnect().await?;

        Ok(())
    }
}

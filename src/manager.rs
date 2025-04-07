use btleplug::api::{Central as _, CentralEvent, Peripheral as _, ScanFilter};
use futures::{StreamExt as _, executor::block_on};
use log::{debug, error, info, warn};
use tokio::sync::broadcast;

use crate::{config::BleDevice, mqtt::MqttAnnouncement};

pub struct Manager {
    adapter: btleplug::platform::Adapter,
    mqtt_client: crate::mqtt::MqttClient,
    mqtt_event_loop: rumqttc::EventLoop,
    devices: Vec<BleDevice>,
}

impl Manager {
    pub fn new(
        adapter: btleplug::platform::Adapter,
        mqtt_client: crate::mqtt::MqttClient,
        mqtt_event_loop: rumqttc::EventLoop,
        devices: Vec<BleDevice>,
    ) -> Self {
        Manager {
            adapter,
            mqtt_client,
            mqtt_event_loop,
            devices,
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
            if let Err(err) = block_on(handle_btle_events(&self.adapter, rx, self.devices)) {
                error!("Error handling BLE events: {:?}", err);
            }
            debug!("Done handling BLE events");
        });

        if let Err(err) = btle_handle.join() {
            error!("Error handling BLE events: {:?}", err);
        }
        debug!("Exiting manager event loop");

        self.mqtt_client.disconnect().await?;

        Ok(())
    }
}

async fn handle_btle_events(
    adapter: &btleplug::platform::Adapter,
    mut rx: broadcast::Receiver<crate::mqtt::MqttAnnouncement>,
    devices: Vec<BleDevice>,
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
                    MqttAnnouncement::ScanArrive => {
                        info!("Received arrival scan request");
                        adapter.start_scan(ScanFilter::default()).await?;
                    }
                    MqttAnnouncement::ScanDepart => {
                        info!("Received departure request");
                        unimplemented!("Need to handle this");
                    }
                }
            }
            // TODO: Selection channel for timers
            // Handle BLE events
            event = events.next() => {
                match event {
                    Some(CentralEvent::DeviceDiscovered(id)) => {
                        let peripheral = adapter.peripheral(&id).await?;
                        let properties = peripheral.properties().await?;
                        match matching_device(&devices, properties) {
                            Some(_device) => {
                                // TODO: send MQTT message
                                // Start a timer for when it falls out of announcement
                                unimplemented!("Need to handle this");
                            }
                            None => {}
                        }
                    }
                    Some(_) => {}
                    None => {
                        warn!("No more BLE events");
                        event_stream_closed = true;
                    }
                }
            }
            else => {}
        }
    }
    Ok(())
}

fn matching_device(
    devices: &[BleDevice],
    properties: Option<btleplug::api::PeripheralProperties>,
) -> Option<&BleDevice> {
    match properties {
        Some(props) => {
            let name = props.local_name.unwrap_or_default();
            if let Some(matching_device) = devices
                .iter()
                .find(|d| d.address.bytes() == props.address.as_ref())
            {
                info!("Discovered device {} ({})", matching_device.address, name);
                return Some(matching_device);
            } else {
                debug!(
                    "Discovered device but not interested in MAC {} ({})",
                    props.address, name,
                );
                return None;
            }
        }
        None => {
            warn!("No properties for discovered device");
            return None;
        }
    }
}

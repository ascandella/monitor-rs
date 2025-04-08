use std::collections::HashSet;

use btleplug::api::{Central as _, CentralEvent, Peripheral as _, ScanFilter};
use futures::StreamExt as _;
use log::{debug, error, info, warn};
use tokio::sync::broadcast;

use crate::{config::BleDevice, scanner::Scanner};

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

        let (mqtt_tx, mqtt_rx) = broadcast::channel(10);

        let mut scanner = Scanner::new(mqtt_rx);

        // Handle incoming MQTT messages (e.g. arrival scan requests)
        tokio::task::spawn(async move {
            // TODO: Need to pass ability to trigger a BTLE scan to the event loop
            crate::mqtt::MqttClient::event_loop(&mut self.mqtt_event_loop, mqtt_tx).await;
        });

        tokio::task::spawn(async move {
            if let Err(err) = scanner.run().await {
                error!("Error handling scanner events: {:?}", err);
            }
        });

        // Run on a separate thread as these currently block
        let btle_handle = tokio::task::spawn(async move {
            // TODO: Need to pass the ability to publish MQTT messages to this function
            if let Err(err) = handle_btle_events(&self.adapter, self.devices).await {
                error!("Error handling BLE events: {:?}", err);
            }
            debug!("Done handling BLE events");
        });

        if let Err(err) = btle_handle.await {
            error!("Error handling BLE events: {:?}", err);
        }
        debug!("Exiting manager event loop");

        self.mqtt_client.disconnect().await?;

        Ok(())
    }
}

async fn handle_btle_events(
    adapter: &btleplug::platform::Adapter,
    devices: Vec<BleDevice>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut events = adapter.events().await?;

    let mut event_stream_closed = false;

    let device_filters = devices
        .iter()
        .flat_map(|device| {
            device
                .manufacturer
                .as_ref()
                .map(|manufacturer| manufacturer.company_ids())
        })
        .flatten()
        .collect::<HashSet<_>>();

    loop {
        if event_stream_closed {
            break;
        }
        match events.next().await {
            Some(CentralEvent::DeviceDiscovered(id)) => {
                let peripheral = adapter.peripheral(&id).await?;
                let properties = peripheral.properties().await?;

                if matching_device(&device_filters, properties) {
                    // TODO: send MQTT message
                    // Start a timer for when it falls out of announcement
                    unimplemented!("Need to handle this");
                }
            }
            Some(_) => {}
            None => {
                warn!("No more BLE events");
                event_stream_closed = true;
            }
        }
    }
    Ok(())
}

fn matching_device(
    company_ids: &HashSet<u16>,
    properties: Option<btleplug::api::PeripheralProperties>,
) -> bool {
    match properties {
        Some(props) => {
            let name = props.local_name.unwrap_or_default();
            let manufacturer_data = props.manufacturer_data;
            let manufacturer_id = manufacturer_data.keys().find(|id| company_ids.contains(id));

            if let Some(manufacturer_id) = manufacturer_id {
                info!(
                    "Discovered device passing manufacturer {} ({}) [{}]",
                    props.address, name, manufacturer_id
                );
                true
            } else {
                debug!(
                    "Discovered device but not interested in manufacturer {} ({})",
                    props.address, name
                );
                false
            }
        }
        None => {
            warn!("No properties for discovered device");
            false
        }
    }
}

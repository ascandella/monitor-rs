use std::collections::HashSet;

use anyhow::Context as _;
use btleplug::api::{Central as _, CentralEvent, Peripheral as _, ScanFilter};
use futures::StreamExt as _;
use log::{debug, error, warn};
use tokio::sync::broadcast;

use crate::{
    config::{AppConfig, BleDevice},
    messages::{DeviceAnnouncement, DevicePresence, StateAnnouncement},
    mqtt::MqttClient,
    scanner::Scanner,
};

pub struct Manager {
    cfg: AppConfig,
    adapter: btleplug::platform::Adapter,
    mqtt_client: MqttClient,
    mqtt_event_loop: rumqttc::EventLoop,
    devices: Vec<BleDevice>,
}

impl Manager {
    pub fn new(
        cfg: &AppConfig,
        adapter: btleplug::platform::Adapter,
        mqtt_client: MqttClient,
        mqtt_event_loop: rumqttc::EventLoop,
    ) -> Self {
        Manager {
            cfg: cfg.clone(),
            adapter,
            mqtt_client,
            mqtt_event_loop,
            devices: cfg.devices.clone().unwrap_or_default().clone(),
        }
    }

    pub async fn run_loop(mut self) -> anyhow::Result<()> {
        self.adapter
            .start_scan(ScanFilter::default())
            .await
            .context("start adapter scan")?;

        let (tx, rx) = broadcast::channel(10);
        let (announce_tx, announce_rx) = broadcast::channel(10);

        let btle_tx = tx.clone();

        let mut scanner = Scanner::new(
            &self.cfg.scan.unwrap_or_default(),
            rx,
            announce_tx,
            &self.devices,
        );

        let mqtt_client = self.mqtt_client.clone();

        // Handle incoming MQTT messages (e.g. arrival scan requests)
        tokio::task::spawn(async move {
            mqtt_client.event_loop(&mut self.mqtt_event_loop, tx).await;
        });

        tokio::task::spawn(async move {
            if let Err(err) = scanner.run().await {
                error!("Error handling scanner events: {:?}", err);
            }
            debug!("Done scanning devices");
        });

        tokio::task::spawn(async move {
            if let Err(err) = announce_scan_results(announce_rx, &self.mqtt_client).await {
                error!("Error handling scan results: {:?}", err);
            }
            debug!("Done announcing scan results");
        });

        // Run on a separate thread as these currently block
        let btle_handle = tokio::task::spawn(async move {
            if let Err(err) = handle_btle_events(&self.adapter, self.devices, btle_tx).await {
                error!("Error handling BLE events: {:?}", err);
            }
            debug!("Done handling BLE events");
        });

        if let Err(err) = btle_handle.await {
            error!("Error handling BLE events: {:?}", err);
        }
        debug!("Exiting manager event loop");

        Ok(())
    }
}

async fn announce_scan_results(
    mut announce_rx: broadcast::Receiver<DeviceAnnouncement>,
    mqtt_client: &MqttClient,
) -> anyhow::Result<()> {
    debug!("Start announce scan results loop");
    loop {
        match announce_rx.recv().await {
            Ok(msg) => match msg {
                DeviceAnnouncement {
                    presence: DevicePresence::Absent,
                    mac_address,
                    name,
                } => mqtt_client.announce_device(&name, mac_address, 0).await?,
                DeviceAnnouncement {
                    presence: DevicePresence::Present(confidence),
                    mac_address,
                    name,
                } => {
                    mqtt_client
                        .announce_device(&name, mac_address, confidence)
                        .await?;
                }
            },
            Err(broadcast::error::RecvError::Closed) => {
                debug!("Receiver closed");
                break;
            }
            Err(broadcast::error::RecvError::Lagged(_)) => {
                debug!("Receiver lagged");
            }
        }
    }

    mqtt_client.disconnect().await?;
    Ok(())
}

async fn handle_btle_events(
    adapter: &btleplug::platform::Adapter,
    devices: Vec<BleDevice>,
    tx: broadcast::Sender<StateAnnouncement>,
) -> anyhow::Result<()> {
    let mut events = adapter.events().await.context("start event stream")?;

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
                let peripheral = adapter.peripheral(&id).await.context("get peripheral")?;
                let properties = peripheral
                    .properties()
                    .await
                    .context("get device properties")?;

                if matching_device(&device_filters, properties) {
                    if let Err(err) = tx.send(StateAnnouncement::DeviceTrigger) {
                        error!("Error sending scan arrival message: {:?}", err);
                    }
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
            let name = props
                .local_name
                .map(|name| format!(" name: {}", name))
                .unwrap_or_default();
            let manufacturer_data = props.manufacturer_data;
            let manufacturer_id = manufacturer_data.keys().find(|id| company_ids.contains(id));

            if let Some(manufacturer_id) = manufacturer_id {
                debug!(
                    "Discovered device passing manufacturer filter {}{} [{}]",
                    props.address, name, manufacturer_id
                );
                true
            } else {
                debug!(
                    "Discovered device but not interested in manufacturer {}{}",
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

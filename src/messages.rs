#[derive(Clone, Debug)]
pub enum StateAnnouncement {
    DeviceTrigger,
    ScanArrive,
    ScanDepart,
    CheckStillPresent(/* device name */ String),
}

#[derive(Clone, Debug)]
pub enum DevicePresence {
    Present(/* confidence */ u8),
    Absent,
}

#[derive(Clone, Debug)]
pub struct DeviceAnnouncement {
    pub name: String,
    pub mac_address: String,
    pub presence: DevicePresence,
}

#[derive(Clone, Debug)]
pub enum StateAnnouncement {
    DeviceTrigger,
    ScanArrive,
    ScanDepart,
}

#[derive(Clone, Debug)]
pub enum DevicePresence {
    Present(/* confidence */ u8),
    Absent,
}

#[derive(Clone, Debug)]
pub struct DeviceAnnouncement {
    pub name: String,
    pub presence: DevicePresence,
}

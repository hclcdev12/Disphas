//! Fictional drives, one per verdict state, to develop the UI without
//! hardware. Behind the `demo` feature, refused in release builds.
//! These drives exist only in memory: nothing here reaches a device.

#[cfg(not(debug_assertions))]
compile_error!("the `demo` feature is for development builds only");

use oblivio_core::profile::{
    AtaProfile, AtaSanitizeCaps, AtaSecurity, AtaSmart, CriticalWarning, DeviceIdentity,
    DeviceProfile, Inspection, NvmeFormatCaps, NvmeHealth, NvmeProfile, NvmeSanitizeCaps,
    ProtectionFacts, QuirkFlags,
};
use oblivio_protocol::command::DeviceIdentity as WireIdentity;

use crate::backend::{Device, DeviceBackend};
use crate::convert;

/// Backend over fictional drives.
#[derive(Debug, Clone)]
pub struct DemoDevices {
    devices: Vec<Device>,
}

fn identity(model: &str, serial: &str, eui64: Option<&str>, wwn: Option<&str>) -> DeviceIdentity {
    DeviceIdentity {
        model: model.to_owned(),
        serial: serial.to_owned(),
        firmware: "DEMO01".to_owned(),
        wwn: wwn.map(str::to_owned),
        eui64: eui64.map(str::to_owned),
        nguid: None,
    }
}

fn device(node: &str, capacity: u64, identity: DeviceIdentity, inspection: Inspection) -> Device {
    Device {
        node_hint: Some(node.to_owned()),
        capacity_bytes: Some(capacity),
        profile: DeviceProfile {
            identity,
            protection: ProtectionFacts::default(),
            inspection,
            failed_unblock_attempts: 0,
            quirks: QuirkFlags::default(),
        },
    }
}

fn healthy_nvme() -> NvmeProfile {
    NvmeProfile {
        sanitize: NvmeSanitizeCaps {
            block_erase: true,
            crypto_erase: true,
        },
        format: NvmeFormatCaps {
            supported: true,
            crypto_erase: true,
        },
        ..NvmeProfile::default()
    }
}

impl Default for DemoDevices {
    fn default() -> Self {
        let mut system = device(
            "/dev/nvme0n1",
            1_024_209_543_168,
            identity(
                "DEMO NVMe System 1TB",
                "DEMO-SYS-0001",
                Some("00a07501deadbeef"),
                None,
            ),
            Inspection::Nvme(healthy_nvme()),
        );
        system.profile.protection.hosts_system = true;
        system.profile.protection.has_mounted_partition = true;

        let ready = device(
            "/dev/nvme1n1",
            2_000_398_934_016,
            identity(
                "DEMO NVMe Data 2TB",
                "DEMO-NVME-4567",
                Some("0011223344556677"),
                None,
            ),
            Inspection::Nvme(healthy_nvme()),
        );

        let worn = device(
            "/dev/sda",
            4_000_787_030_016,
            identity(
                "DEMO HDD 4TB",
                "DEMO-HDD-8C21",
                None,
                Some("5000c500deadbeef"),
            ),
            Inspection::Ata(AtaProfile {
                solid_state: false,
                sanitize: AtaSanitizeCaps {
                    overwrite: true,
                    ..AtaSanitizeCaps::default()
                },
                security: AtaSecurity {
                    supported: true,
                    enhanced_erase: true,
                    ..AtaSecurity::default()
                },
                smart: AtaSmart {
                    reallocated_sectors: Some(72),
                    pending_sectors: Some(0),
                    offline_uncorrectable: Some(0),
                    reported_uncorrectable: Some(2),
                    failing: false,
                },
                ..AtaProfile::default()
            }),
        );

        let frozen = device(
            "/dev/sdb",
            500_107_862_016,
            identity(
                "DEMO SATA SSD 500GB",
                "DEMO-SSD-F0Z3",
                None,
                Some("5002538e00000001"),
            ),
            Inspection::Ata(AtaProfile {
                solid_state: true,
                security: AtaSecurity {
                    supported: true,
                    enhanced_erase: true,
                    frozen: true,
                    ..AtaSecurity::default()
                },
                smart: AtaSmart {
                    reallocated_sectors: Some(0),
                    ..AtaSmart::default()
                },
                ..AtaProfile::default()
            }),
        );

        let read_only = device(
            "/dev/nvme2n1",
            256_060_514_304,
            identity(
                "DEMO NVMe Worn 256GB",
                "DEMO-RO-0BAD",
                Some("00a07501feedface"),
                None,
            ),
            Inspection::Nvme(NvmeProfile {
                health: NvmeHealth {
                    critical_warning: CriticalWarning(CriticalWarning::READ_ONLY),
                    percentage_used: 255,
                    media_errors: 41,
                },
                ..healthy_nvme()
            }),
        );

        let mut mute = device(
            "/dev/sdc",
            1_000_204_886_016,
            identity("DEMO Unresponsive 1TB", "DEMO-MUTE-0N1X", None, None),
            Inspection::Failed,
        );
        mute.capacity_bytes = None;

        Self {
            devices: vec![system, ready, worn, frozen, read_only, mute],
        }
    }
}

impl DeviceBackend for DemoDevices {
    fn list(&mut self) -> Vec<Device> {
        self.devices.clone()
    }

    fn find(&mut self, identity: &WireIdentity) -> Option<Device> {
        self.devices
            .iter()
            .find(|device| convert::identity(&device.profile.identity) == *identity)
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oblivio_core::{RuleSet, Verdict, decide};

    #[test]
    fn demo_covers_every_state() {
        let states: Vec<&str> = DemoDevices::default()
            .list()
            .iter()
            .map(
                |device| match decide(&device.profile, &RuleSet::baseline()) {
                    Verdict::Protected { .. } => "protected",
                    Verdict::Blocked { .. } => "blocked",
                    Verdict::SayNo {
                        forcible: Some(_), ..
                    } => "say-no-forcible",
                    Verdict::SayNo { .. } => "say-no",
                    Verdict::Ready(_) => "ready",
                },
            )
            .collect();
        assert_eq!(
            states,
            [
                "protected",
                "ready",
                "ready",
                "blocked",
                "say-no",
                "say-no-forcible"
            ]
        );
    }
}

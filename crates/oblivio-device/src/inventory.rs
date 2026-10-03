//! Read-only inventory of NVMe and SATA drives and of their protections (P1 to P5).
//!
//! Reads `/sys` and `/proc` only: no `/dev` node is opened, no ioctl is sent.
//! Fails closed: whatever cannot be proven free is reported protected, or the
//! whole scan fails.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use oblivio_core::profile::ProtectionFacts;
use thiserror::Error;

use crate::procfs::{self, MountEntry};
use crate::sysfs::{self, DevNum};

/// Mount points whose drive is the system drive (P1).
const SYSTEM_MOUNT_POINTS: &[&str] = &["/", "/boot", "/boot/efi", "/efi"];

/// Deepest device stack followed through `slaves/` (dm-crypt on LVM on md…).
const MAX_STACK_DEPTH: usize = 16;

/// Where the system files live. Tests point these at a fake tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemPaths {
    /// sysfs mount, normally `/sys`.
    pub sys: PathBuf,
    /// procfs mount, normally `/proc`.
    pub proc: PathBuf,
    /// devtmpfs mount, normally `/dev`. Only symlinks are resolved there.
    pub dev: PathBuf,
}

impl Default for SystemPaths {
    fn default() -> Self {
        Self {
            sys: PathBuf::from("/sys"),
            proc: PathBuf::from("/proc"),
            dev: PathBuf::from("/dev"),
        }
    }
}

/// How the drive is attached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Transport {
    /// NVMe.
    Nvme,
    /// SATA through libata.
    Ata,
    /// USB bridge (out of scope in V1).
    Usb,
    /// Anything else, or undetermined.
    Other,
}

/// Identity hints read from sysfs. The certified identity comes from IDENTIFY.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SysfsIdentity {
    /// Model string.
    pub model: Option<String>,
    /// Serial number.
    pub serial: Option<String>,
    /// Firmware revision (truncated to 4 characters for SATA by the kernel).
    pub firmware: Option<String>,
    /// World wide identifier as exposed by the kernel (`eui.…`, `naa.…`, `t10.…`).
    pub wwid: Option<String>,
}

/// A drive found by the inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockDevice {
    /// Kernel name, such as `nvme0n1` or `sda`.
    pub name: String,
    /// Device number.
    pub dev: DevNum,
    /// How the drive is attached.
    pub transport: Transport,
    /// Capacity in bytes.
    pub size_bytes: u64,
    /// Rotational medium, if the kernel knows.
    pub rotational: Option<bool>,
    /// Identity hints.
    pub identity: SysfsIdentity,
    /// Partition names, sorted.
    pub partitions: Vec<String>,
    /// Protections found (P1 to P5).
    pub protection: ProtectionFacts,
}

/// Why the inventory cannot be trusted.
#[derive(Debug, Error)]
pub enum ScanError {
    /// A required file could not be read.
    #[error("cannot read {path}: {source}")]
    Read {
        /// The file.
        path: PathBuf,
        /// The cause.
        source: io::Error,
    },
    /// A required file has an unexpected format.
    #[error("cannot parse {path}: {reason}")]
    Parse {
        /// The file.
        path: PathBuf,
        /// What is wrong.
        reason: String,
    },
    /// A mounted or swap device could not be traced back to a drive.
    #[error("cannot trace {0} back to a drive")]
    Untraceable(String),
    /// The root filesystem could not be traced back to any drive, so the
    /// system drive is unknown.
    #[error("the drive holding / is unknown")]
    RootNotResolved,
}

/// Lists every NVMe and SATA drive with its protections.
pub fn scan(paths: &SystemPaths) -> Result<Vec<BlockDevice>, ScanError> {
    let block_dir = paths.sys.join("block");
    let names = sysfs::entry_names(&block_dir).map_err(|source| read_error(&block_dir, source))?;

    let mut devices = Vec::new();
    let mut owner = HashMap::new(); // disk or partition name -> disk name
    for name in names.into_iter().filter(|name| is_supported_drive(name)) {
        let device = read_device(&block_dir.join(&name), &name)?;
        owner.insert(name.clone(), name.clone());
        for partition in &device.partitions {
            owner.insert(partition.clone(), name.clone());
        }
        devices.push(device);
    }

    let resolver = Resolver::new(paths, owner)?;
    let mut protection: BTreeMap<String, ProtectionFacts> = BTreeMap::new();

    // P1, P2: mounts, traced through device stacks.
    let mountinfo_path = paths.proc.join("self/mountinfo");
    let mounts = procfs::parse_mountinfo(&read_required(&mountinfo_path)?)
        .map_err(|error| parse_error(&mountinfo_path, error))?;
    let mut root_resolved = false;
    for mount in &mounts {
        let disks = resolver.mount_disks(mount)?;
        let system = SYSTEM_MOUNT_POINTS.contains(&mount.mount_point.as_str());
        root_resolved |= mount.mount_point == "/" && !disks.is_empty();
        for disk in disks {
            let facts = protection.entry(disk).or_default();
            facts.has_mounted_partition = true;
            facts.hosts_system |= system;
        }
    }
    if !root_resolved {
        return Err(ScanError::RootNotResolved);
    }

    // P3: swap partitions. Swap files live on a mounted filesystem: already P2.
    let swaps_path = paths.proc.join("swaps");
    let swaps = procfs::parse_swaps(&read_required(&swaps_path)?)
        .map_err(|error| parse_error(&swaps_path, error))?;
    for swap in swaps.iter().filter(|swap| swap.kind != "file") {
        let name = resolver
            .dev_path_name(&swap.path)
            .ok_or_else(|| ScanError::Untraceable(swap.path.clone()))?;
        for disk in resolver.disks_of(&name) {
            protection.entry(disk).or_default().has_active_swap = true;
        }
    }

    // P4: holders on the drive or one of its partitions.
    for device in &devices {
        if has_holders(&block_dir.join(&device.name), &device.partitions) {
            protection
                .entry(device.name.clone())
                .or_default()
                .is_volume_member = true;
        }
    }

    // P5: a protected namespace protects its whole NVMe controller.
    let protected_controllers: BTreeSet<String> = devices
        .iter()
        .filter(|device| protection.get(&device.name).is_some_and(is_protected))
        .filter_map(|device| nvme_controller(&device.name))
        .collect();
    for device in &mut devices {
        let mut facts = protection.get(&device.name).copied().unwrap_or_default();
        facts.sibling_namespace_protected = !is_protected(&facts)
            && nvme_controller(&device.name)
                .is_some_and(|controller| protected_controllers.contains(&controller));
        device.protection = facts;
    }
    Ok(devices)
}

/// NVMe namespaces (`nvme0n1`) and SCSI disks (`sda`); nothing else in V1.
/// Hidden multipath paths (`nvme0c0n1`), partitions, loop, dm, md… are excluded.
fn is_supported_drive(name: &str) -> bool {
    if let Some(rest) = name.strip_prefix("nvme") {
        return match rest.split_once('n') {
            Some((controller, namespace)) => is_digits(controller) && is_digits(namespace),
            None => false,
        };
    }
    name.strip_prefix("sd").is_some_and(|letters| {
        !letters.is_empty() && letters.bytes().all(|b| b.is_ascii_lowercase())
    })
}

fn is_digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit())
}

/// Controller of an NVMe namespace: `nvme0n2` -> `nvme0`.
fn nvme_controller(name: &str) -> Option<String> {
    let rest = name.strip_prefix("nvme")?;
    let (controller, _) = rest.split_once('n')?;
    Some(format!("nvme{controller}"))
}

fn is_protected(facts: &ProtectionFacts) -> bool {
    facts.hosts_system
        || facts.has_mounted_partition
        || facts.has_active_swap
        || facts.is_volume_member
}

fn read_device(dir: &Path, name: &str) -> Result<BlockDevice, ScanError> {
    let dev_path = dir.join("dev");
    let dev = read_required(&dev_path)?
        .parse()
        .map_err(|()| parse_error(&dev_path, "invalid major:minor"))?;
    let size_path = dir.join("size");
    let sectors: u64 = read_required(&size_path)?
        .trim()
        .parse()
        .map_err(|_| parse_error(&size_path, "invalid size"))?;

    let partitions = sysfs::entry_names(dir)
        .map_err(|source| read_error(dir, source))?
        .into_iter()
        .filter(|entry| dir.join(entry).join("partition").is_file())
        .collect();

    let transport = if name.starts_with("nvme") {
        Transport::Nvme
    } else {
        let real = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
        let real = real.to_string_lossy();
        if real.contains("/usb") {
            Transport::Usb
        } else if real.contains("/ata") {
            Transport::Ata
        } else {
            Transport::Other
        }
    };

    let device = dir.join("device");
    let identity = SysfsIdentity {
        model: sysfs::read_optional(&device.join("model")),
        serial: sysfs::read_optional(&device.join("serial")).or_else(|| {
            fs::read(device.join("vpd_pg80"))
                .ok()
                .and_then(|page| sysfs::vpd_pg80_serial(&page))
        }),
        firmware: sysfs::read_optional(&device.join("firmware_rev"))
            .or_else(|| sysfs::read_optional(&device.join("rev"))),
        wwid: sysfs::read_optional(&dir.join("wwid"))
            .or_else(|| sysfs::read_optional(&device.join("wwid"))),
    };

    Ok(BlockDevice {
        name: name.to_owned(),
        dev,
        transport,
        size_bytes: sectors.saturating_mul(512),
        rotational: sysfs::read_optional(&dir.join("queue/rotational")).map(|flag| flag == "1"),
        identity,
        partitions,
        protection: ProtectionFacts::default(),
    })
}

/// Whether the drive or one of its partitions has holders. An unreadable
/// `holders/` directory counts as held: it cannot be proven free.
fn has_holders(dir: &Path, partitions: &[String]) -> bool {
    std::iter::once(dir.join("holders"))
        .chain(
            partitions
                .iter()
                .map(|partition| dir.join(partition).join("holders")),
        )
        .any(|holders| sysfs::entry_names(&holders).map_or(true, |entries| !entries.is_empty()))
}

/// Traces block devices (partitions, dm, md…) back to the drives under them.
struct Resolver<'a> {
    paths: &'a SystemPaths,
    /// Drive or partition name -> drive name.
    owner: HashMap<String, String>,
    /// Device number -> kernel name, for every block device.
    names: HashMap<DevNum, String>,
}

impl<'a> Resolver<'a> {
    fn new(paths: &'a SystemPaths, owner: HashMap<String, String>) -> Result<Self, ScanError> {
        let class = paths.sys.join("class/block");
        let mut names = HashMap::new();
        for name in sysfs::entry_names(&class).map_err(|source| read_error(&class, source))? {
            let dev_path = class.join(&name).join("dev");
            if let Some(dev) = sysfs::read_optional(&dev_path).and_then(|text| text.parse().ok()) {
                names.insert(dev, name);
            }
        }
        Ok(Self {
            paths,
            owner,
            names,
        })
    }

    /// Drives under a mount. Virtual filesystems (tmpfs, proc, overlay…) have none.
    fn mount_disks(&self, mount: &MountEntry) -> Result<BTreeSet<String>, ScanError> {
        let name = if mount.dev.major != 0 {
            self.names.get(&mount.dev).cloned()
        } else if mount.source.starts_with("/dev/") {
            // btrfs and a few others report an anonymous device number.
            Some(
                self.dev_path_name(&mount.source)
                    .ok_or_else(|| ScanError::Untraceable(mount.source.clone()))?,
            )
        } else {
            None
        };
        Ok(name.map(|name| self.disks_of(&name)).unwrap_or_default())
    }

    /// Kernel name behind a `/dev/…` path, following symlinks such as
    /// `/dev/mapper/root` -> `dm-0`. Only resolves links, never opens the node.
    fn dev_path_name(&self, path: &str) -> Option<String> {
        let relative = path.strip_prefix("/dev/")?;
        let real = fs::canonicalize(self.paths.dev.join(relative)).ok()?;
        Some(real.file_name()?.to_string_lossy().into_owned())
    }

    /// Drives under a block device, through `slaves/` for stacked devices.
    fn disks_of(&self, name: &str) -> BTreeSet<String> {
        let mut disks = BTreeSet::new();
        self.collect_disks(name, 0, &mut disks);
        disks
    }

    fn collect_disks(&self, name: &str, depth: usize, disks: &mut BTreeSet<String>) {
        if let Some(disk) = self.owner.get(name) {
            disks.insert(disk.clone());
            return;
        }
        if depth >= MAX_STACK_DEPTH {
            return;
        }
        let slaves = self.paths.sys.join("class/block").join(name).join("slaves");
        for slave in sysfs::entry_names(&slaves).unwrap_or_default() {
            self.collect_disks(&slave, depth + 1, disks);
        }
    }
}

fn read_required(path: &Path) -> Result<String, ScanError> {
    fs::read_to_string(path).map_err(|source| read_error(path, source))
}

fn read_error(path: &Path, source: io::Error) -> ScanError {
    ScanError::Read {
        path: path.to_path_buf(),
        source,
    }
}

fn parse_error(path: &Path, reason: impl ToString) -> ScanError {
    ScanError::Parse {
        path: path.to_path_buf(),
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    /// A fake `/sys`, `/proc` and `/dev` tree.
    struct Fixture {
        root: TempDir,
        mounts: Vec<String>,
        swaps: Vec<String>,
    }

    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir().expect("temporary directory");
            for dir in [
                "sys/block",
                "sys/class/block",
                "sys/devices",
                "proc/self",
                "dev/mapper",
            ] {
                fs::create_dir_all(root.path().join(dir)).expect("fixture directory");
            }
            Self {
                root,
                mounts: Vec::new(),
                swaps: Vec::new(),
            }
        }

        fn path(&self, relative: &str) -> PathBuf {
            self.root.path().join(relative)
        }

        /// Writes a file the way the kernel does: non-empty content ends with a newline.
        fn write(&self, relative: &str, content: &str) {
            let path = self.path(relative);
            fs::create_dir_all(path.parent().expect("parent")).expect("parent directory");
            let mut content = content.to_owned();
            if !content.is_empty() && !content.ends_with('\n') {
                content.push('\n');
            }
            fs::write(path, content).expect("fixture file");
        }

        fn class_entry(&self, name: &str, dev: &str) {
            self.write(&format!("sys/class/block/{name}/dev"), dev);
            fs::create_dir_all(self.path(&format!("sys/class/block/{name}/slaves")))
                .expect("slaves");
            self.write(&format!("dev/{name}"), "");
        }

        /// A drive under `sys/devices/<bus>/…`, linked from `sys/block`.
        fn disk(&self, name: &str, dev: &str, bus: &str) -> &Self {
            let real = format!("sys/devices/{bus}/block/{name}");
            self.write(&format!("{real}/dev"), dev);
            self.write(&format!("{real}/size"), "1953525168");
            self.write(&format!("{real}/queue/rotational"), "0");
            self.write(&format!("{real}/device/model"), "Fictional Drive  ");
            self.write(&format!("{real}/device/serial"), &format!("SN-{name}"));
            fs::create_dir_all(self.path(&format!("{real}/holders"))).expect("holders");
            symlink(self.path(&real), self.path(&format!("sys/block/{name}"))).expect("link");
            self.class_entry(name, dev);
            self
        }

        fn partition(&self, disk: &str, name: &str, dev: &str) -> &Self {
            let dir = format!("sys/block/{disk}/{name}");
            self.write(&format!("{dir}/partition"), "1");
            self.write(&format!("{dir}/dev"), dev);
            fs::create_dir_all(self.path(&format!("{dir}/holders"))).expect("holders");
            self.class_entry(name, dev);
            self
        }

        /// A stacked device (dm, md) built on `slaves`, held by them.
        fn stacked(&self, name: &str, dev: &str, slaves: &[(&str, &str)]) -> &Self {
            self.class_entry(name, dev);
            for (slave, holder_dir) in slaves {
                fs::create_dir_all(self.path(&format!("sys/class/block/{name}/slaves/{slave}")))
                    .expect("slave");
                fs::create_dir_all(self.path(&format!("{holder_dir}/holders/{name}")))
                    .expect("holder");
            }
            self
        }

        fn mount(
            &mut self,
            dev: &str,
            mount_point: &str,
            fs_type: &str,
            source: &str,
        ) -> &mut Self {
            let id = self.mounts.len() + 20;
            self.mounts.push(format!(
                "{id} 1 {dev} / {mount_point} rw,relatime shared:1 - {fs_type} {source} rw"
            ));
            self
        }

        fn swap(&mut self, path: &str) -> &mut Self {
            self.swaps.push(format!("{path} partition 8388604 0 -2"));
            self
        }

        fn scan(&self) -> Result<Vec<BlockDevice>, ScanError> {
            self.write("proc/self/mountinfo", &(self.mounts.join("\n") + "\n"));
            self.write(
                "proc/swaps",
                &format!(
                    "Filename Type Size Used Priority\n{}\n",
                    self.swaps.join("\n")
                ),
            );
            scan(&SystemPaths {
                sys: self.path("sys"),
                proc: self.path("proc"),
                dev: self.path("dev"),
            })
        }
    }

    /// Two NVMe drives: the system on nvme0n1 (root + EFI), nvme1n1 free.
    fn two_nvme() -> Fixture {
        let mut fixture = Fixture::new();
        fixture
            .disk("nvme0n1", "259:0", "pci0000:00/nvme/nvme0")
            .partition("nvme0n1", "nvme0n1p1", "259:1")
            .partition("nvme0n1", "nvme0n1p2", "259:2")
            .disk("nvme1n1", "259:3", "pci0000:00/nvme/nvme1");
        fixture
            .mount("259:2", "/", "ext4", "/dev/nvme0n1p2")
            .mount("259:1", "/boot/efi", "vfat", "/dev/nvme0n1p1")
            .mount("0:22", "/proc", "proc", "proc")
            .mount("0:25", "/tmp", "tmpfs", "tmpfs");
        fixture
    }

    fn protection_of(devices: &[BlockDevice], name: &str) -> ProtectionFacts {
        devices
            .iter()
            .find(|device| device.name == name)
            .map(|device| device.protection)
            .unwrap_or_else(|| panic!("{name} not listed"))
    }

    fn only(facts: impl FnOnce(&mut ProtectionFacts)) -> ProtectionFacts {
        let mut expected = ProtectionFacts::default();
        facts(&mut expected);
        expected
    }

    #[test]
    fn lists_supported_drives_with_identity() {
        let devices = two_nvme().scan().expect("scan");
        let names: Vec<&str> = devices.iter().map(|device| device.name.as_str()).collect();
        assert_eq!(names, ["nvme0n1", "nvme1n1"]);
        let free = &devices[1];
        assert_eq!(free.transport, Transport::Nvme);
        assert_eq!(free.size_bytes, 1_953_525_168 * 512);
        assert_eq!(free.rotational, Some(false));
        assert_eq!(free.identity.model.as_deref(), Some("Fictional Drive"));
        assert_eq!(free.identity.serial.as_deref(), Some("SN-nvme1n1"));
        assert_eq!(devices[0].partitions, ["nvme0n1p1", "nvme0n1p2"]);
    }

    #[test]
    fn ignores_unsupported_block_devices() {
        for name in [
            "loop0",
            "dm-0",
            "md0",
            "sr0",
            "zram0",
            "mmcblk0",
            "nvme0c0n1",
            "sda1",
            "sd",
        ] {
            assert!(!is_supported_drive(name), "{name}");
        }
        for name in ["sda", "sdab", "nvme0n1", "nvme12n3"] {
            assert!(is_supported_drive(name), "{name}");
        }
    }

    #[test]
    fn p1_and_p2_system_drive() {
        let devices = two_nvme().scan().expect("scan");
        assert_eq!(
            protection_of(&devices, "nvme0n1"),
            only(|p| {
                p.hosts_system = true;
                p.has_mounted_partition = true;
            })
        );
        assert_eq!(
            protection_of(&devices, "nvme1n1"),
            ProtectionFacts::default()
        );
    }

    #[test]
    fn p2_data_mount_is_not_a_system_drive() {
        let mut fixture = two_nvme();
        fixture.mount("259:3", "/mnt/data", "xfs", "/dev/nvme1n1");
        let devices = fixture.scan().expect("scan");
        assert_eq!(
            protection_of(&devices, "nvme1n1"),
            only(|p| p.has_mounted_partition = true)
        );
    }

    #[test]
    fn p1_root_on_luks_over_lvm_is_traced_to_the_drive() {
        let mut fixture = Fixture::new();
        fixture
            .disk("sda", "8:0", "pci0000:00/ata1/host0/target0:0:0/0:0:0:0")
            .partition("sda", "sda1", "8:1")
            .partition("sda", "sda2", "8:2")
            .stacked("dm-0", "253:0", &[("sda2", "sys/block/sda/sda2")])
            .stacked("dm-1", "253:1", &[("dm-0", "sys/class/block/dm-0")])
            .disk("sdb", "8:16", "pci0000:00/ata2/host1/target1:0:0/1:0:0:0");
        symlink("../dm-1", fixture.path("dev/mapper/vg-root")).expect("mapper link");
        fixture
            .mount("253:1", "/", "ext4", "/dev/mapper/vg-root")
            .mount("8:1", "/boot", "ext4", "/dev/sda1");
        let devices = fixture.scan().expect("scan");
        let system = protection_of(&devices, "sda");
        assert!(system.hosts_system && system.has_mounted_partition && system.is_volume_member);
        assert_eq!(protection_of(&devices, "sdb"), ProtectionFacts::default());
        assert_eq!(devices[0].transport, Transport::Ata);
    }

    #[test]
    fn p1_btrfs_root_with_anonymous_device_number_is_traced_by_source() {
        let mut fixture = Fixture::new();
        fixture
            .disk("nvme0n1", "259:0", "nvme0")
            .partition("nvme0n1", "nvme0n1p2", "259:2");
        fixture.mount("0:31", "/", "btrfs", "/dev/nvme0n1p2");
        let devices = fixture.scan().expect("scan");
        assert!(protection_of(&devices, "nvme0n1").hosts_system);
    }

    #[test]
    fn p3_swap_partition() {
        let mut fixture = two_nvme();
        fixture.partition("nvme1n1", "nvme1n1p1", "259:4");
        fixture.swap("/dev/nvme1n1p1");
        let devices = fixture.scan().expect("scan");
        assert_eq!(
            protection_of(&devices, "nvme1n1"),
            only(|p| p.has_active_swap = true)
        );
    }

    #[test]
    fn p4_volume_member_without_mount() {
        let fixture = two_nvme();
        fixture.partition("nvme1n1", "nvme1n1p1", "259:4").stacked(
            "md0",
            "9:0",
            &[("nvme1n1p1", "sys/block/nvme1n1/nvme1n1p1")],
        );
        let devices = fixture.scan().expect("scan");
        assert_eq!(
            protection_of(&devices, "nvme1n1"),
            only(|p| p.is_volume_member = true)
        );
    }

    #[test]
    fn p4_unreadable_holders_count_as_held() {
        let fixture = two_nvme();
        fs::remove_dir(fixture.path("sys/devices/pci0000:00/nvme/nvme1/block/nvme1n1/holders"))
            .expect("remove holders");
        let devices = fixture.scan().expect("scan");
        assert!(protection_of(&devices, "nvme1n1").is_volume_member);
    }

    #[test]
    fn p5_sibling_namespace_of_a_protected_one() {
        let fixture = two_nvme();
        fixture.disk("nvme0n2", "259:5", "pci0000:00/nvme/nvme0b");
        let devices = fixture.scan().expect("scan");
        assert_eq!(
            protection_of(&devices, "nvme0n2"),
            only(|p| p.sibling_namespace_protected = true)
        );
        assert!(!protection_of(&devices, "nvme0n1").sibling_namespace_protected);
        assert_eq!(
            protection_of(&devices, "nvme1n1"),
            ProtectionFacts::default()
        );
    }

    #[test]
    fn usb_bridge_is_reported_as_such() {
        let fixture = two_nvme();
        fixture.disk(
            "sdc",
            "8:32",
            "pci0000:00/usb2/2-1/2-1:1.0/host6/target6:0:0/6:0:0:0",
        );
        let devices = fixture.scan().expect("scan");
        let usb = devices
            .iter()
            .find(|d| d.name == "sdc")
            .expect("sdc listed");
        assert_eq!(usb.transport, Transport::Usb);
    }

    // --- Fail closed.

    #[test]
    fn unknown_root_drive_fails_the_scan() {
        let mut fixture = Fixture::new();
        fixture.disk("sda", "8:0", "ata1");
        fixture.mount("0:30", "/", "zfs", "rpool/ROOT/ubuntu");
        assert!(matches!(fixture.scan(), Err(ScanError::RootNotResolved)));
    }

    #[test]
    fn untraceable_swap_fails_the_scan() {
        let mut fixture = two_nvme();
        fixture.swap("/dev/does-not-exist");
        assert!(matches!(fixture.scan(), Err(ScanError::Untraceable(_))));
    }

    #[test]
    fn unreadable_mountinfo_fails_the_scan() {
        let fixture = two_nvme();
        fixture.scan().expect("scan");
        fs::remove_file(fixture.path("proc/self/mountinfo")).expect("remove mountinfo");
        let result = scan(&SystemPaths {
            sys: fixture.path("sys"),
            proc: fixture.path("proc"),
            dev: fixture.path("dev"),
        });
        assert!(matches!(result, Err(ScanError::Read { .. })));
    }
}

//! Notices drives being plugged in or out, so the list follows without a
//! refresh button. Only the names under `/sys/block` are read, as the normal
//! user: no command ever reaches a drive from here. The helper is asked for
//! an inventory only when that set of names changes, because an inventory
//! queries every drive, the ones being erased included.

use std::collections::BTreeSet;
use std::time::Duration;

/// How often the set of block devices is read.
const PERIOD: Duration = Duration::from_secs(2);

/// Names of the block devices the kernel knows, or `None` when unreadable.
fn block_devices() -> Option<BTreeSet<String>> {
    let entries = std::fs::read_dir("/sys/block").ok()?;
    Some(
        entries
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect(),
    )
}

/// Calls `changed` from a background thread each time a block device
/// appears or disappears.
pub fn watch(changed: impl Fn() + Send + 'static) {
    std::thread::spawn(move || {
        let mut known = block_devices();
        loop {
            std::thread::sleep(PERIOD);
            let now = block_devices();
            if now != known {
                known = now;
                changed();
            }
        }
    });
}

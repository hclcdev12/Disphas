//! Erasure primitives: targets, sample selection, canaries, host overwrite
//! and validation (`docs/moteur_decision.md` §5).

use oblivio_core::method::Method;
use oblivio_core::outcome::Validation;
use oblivio_core::profile::AtaSmart;
use oblivio_device::block::BlockIo;
use sha2::{Digest, Sha256};

/// A drive opened for erasure, after every check passed.
pub trait ErasureTarget: Send {
    /// Block access to the drive.
    fn io(&mut self) -> &mut dyn BlockIo;

    /// Runs a firmware method and returns whether the firmware reports success.
    /// `progress` receives percentages.
    fn firmware(&mut self, method: Method, progress: &mut dyn FnMut(u8)) -> Result<bool, String>;

    /// SMART data read again after the operation, for ATA drives.
    fn smart_after(&mut self) -> Option<AtaSmart>;

    /// Whether firmware methods are simulated (demonstration builds only).
    fn simulated(&self) -> bool;
}

/// Largest write issued at once by a host overwrite.
const CHUNK_BYTES: u64 = 4 << 20;

/// Sample blocks for validation: first, middle and last block, then
/// `random` blocks derived from `seed`, sorted and deduplicated.
pub fn samples(seed: &[u8; 32], block_count: u64, random: u32) -> Vec<u64> {
    if block_count == 0 {
        return Vec::new();
    }
    let mut lbas = vec![0, block_count / 2, block_count - 1];
    for index in 0..random {
        let digest = Sha256::new()
            .chain_update(seed)
            .chain_update(b"sample")
            .chain_update(index.to_le_bytes())
            .finalize();
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&digest[..8]);
        lbas.push(u64::from_le_bytes(bytes) % block_count);
    }
    lbas.sort_unstable();
    lbas.dedup();
    lbas
}

/// The canary written in block `lba` before a firmware erasure: unique to the
/// seed and the block, so a block left untouched is always recognized.
pub fn canary(seed: &[u8; 32], lba: u64, block_size: u64) -> Vec<u8> {
    let digest = Sha256::new()
        .chain_update(seed)
        .chain_update(b"canary")
        .chain_update(lba.to_le_bytes())
        .finalize();
    digest
        .iter()
        .copied()
        .cycle()
        .take(block_size as usize)
        .collect()
}

/// Writes a canary in every sample block and makes them durable.
pub fn write_canaries(io: &mut dyn BlockIo, seed: &[u8; 32], lbas: &[u64]) -> std::io::Result<()> {
    let block_size = io.block_size();
    for &lba in lbas {
        io.write_blocks(lba, &canary(seed, lba, block_size))?;
    }
    io.sync()
}

/// Progress of a host overwrite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostProgress {
    /// Bytes written so far.
    pub bytes_done: u64,
    /// Total bytes to write.
    pub bytes_total: u64,
}

/// Overwrites every block with zeros, then makes it durable.
pub fn host_overwrite(
    io: &mut dyn BlockIo,
    progress: &mut dyn FnMut(HostProgress),
) -> std::io::Result<()> {
    let block_size = io.block_size();
    let total = io.block_count();
    let blocks_per_chunk = (CHUNK_BYTES / block_size).max(1);
    let zeros = vec![0u8; (blocks_per_chunk * block_size) as usize];
    let mut lba = 0;
    while lba < total {
        let count = blocks_per_chunk.min(total - lba);
        io.write_blocks(lba, &zeros[..(count * block_size) as usize])?;
        lba += count;
        progress(HostProgress {
            bytes_done: lba * block_size,
            bytes_total: total * block_size,
        });
    }
    io.sync()
}

/// How the result is validated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Check {
    /// Every sample reads back as zeros (host overwrite).
    Zeros,
    /// No sample still holds its canary (firmware methods).
    NoCanary,
}

/// Validation of the sample blocks.
pub fn validate(io: &mut dyn BlockIo, seed: &[u8; 32], lbas: &[u64], check: Check) -> Validation {
    let block_size = io.block_size();
    let mut buffer = vec![0u8; block_size as usize];
    let mut mismatch = false;
    for &lba in lbas {
        if io.read_blocks(lba, &mut buffer).is_err() {
            return Validation::ReadErrors;
        }
        let failed = match check {
            Check::Zeros => buffer.iter().any(|&byte| byte != 0),
            Check::NoCanary => buffer == canary(seed, lba, block_size),
        };
        mismatch |= failed;
    }
    if mismatch {
        Validation::Mismatch
    } else {
        Validation::Passed
    }
}

/// Fresh random seed from the operating system.
pub fn random_seed() -> Result<[u8; 32], String> {
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).map_err(|error| format!("random generator: {error}"))?;
    Ok(seed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oblivio_device::block::FileImage;

    fn image(blocks: u64) -> (tempfile::TempDir, FileImage) {
        let dir = tempfile::tempdir().expect("temporary directory");
        let image = FileImage::create(&dir.path().join("disk.img"), 512, blocks).expect("image");
        (dir, image)
    }

    #[test]
    fn samples_cover_edges_and_are_reproducible() {
        let seed = [7; 32];
        let lbas = samples(&seed, 1000, 50);
        assert_eq!(lbas.first(), Some(&0));
        assert_eq!(lbas.last(), Some(&999));
        assert!(lbas.contains(&500));
        assert!(lbas.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(lbas, samples(&seed, 1000, 50), "same seed, same samples");
        assert_ne!(lbas, samples(&[8; 32], 1000, 50));
    }

    #[test]
    fn canaries_differ_per_block() {
        assert_ne!(canary(&[1; 32], 0, 512), canary(&[1; 32], 1, 512));
        assert_eq!(canary(&[1; 32], 3, 512).len(), 512);
    }

    #[test]
    fn host_overwrite_then_zero_validation_passes() {
        let (_dir, mut image) = image(20_000);
        let seed = [3; 32];
        let lbas = samples(&seed, image.block_count(), 100);
        write_canaries(&mut image, &seed, &lbas).expect("canaries");
        assert_eq!(
            validate(&mut image, &seed, &lbas, Check::Zeros),
            Validation::Mismatch
        );
        let mut last = None;
        host_overwrite(&mut image, &mut |progress| last = Some(progress)).expect("overwrite");
        assert_eq!(last.map(|p| p.bytes_done), Some(20_000 * 512));
        assert_eq!(
            validate(&mut image, &seed, &lbas, Check::Zeros),
            Validation::Passed
        );
    }

    #[test]
    fn surviving_canary_fails_validation() {
        let (_dir, mut image) = image(1000);
        let seed = [4; 32];
        let lbas = samples(&seed, 1000, 100);
        write_canaries(&mut image, &seed, &lbas).expect("canaries");
        assert_eq!(
            validate(&mut image, &seed, &lbas, Check::NoCanary),
            Validation::Mismatch
        );
        // Erase everything but one sample block.
        host_overwrite(&mut image, &mut |_| {}).expect("overwrite");
        let kept = lbas[lbas.len() / 2];
        image
            .write_blocks(kept, &canary(&seed, kept, 512))
            .expect("restore one canary");
        assert_eq!(
            validate(&mut image, &seed, &lbas, Check::NoCanary),
            Validation::Mismatch
        );
    }

    #[test]
    fn unreadable_sample_is_a_read_error() {
        struct Broken;
        impl BlockIo for Broken {
            fn block_size(&self) -> u64 {
                512
            }
            fn block_count(&self) -> u64 {
                10
            }
            fn read_blocks(&mut self, _: u64, _: &mut [u8]) -> std::io::Result<()> {
                Err(std::io::Error::other("medium error"))
            }
            fn write_blocks(&mut self, _: u64, _: &[u8]) -> std::io::Result<()> {
                Ok(())
            }
            fn sync(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        assert_eq!(
            validate(&mut Broken, &[0; 32], &[0, 5], Check::Zeros),
            Validation::ReadErrors
        );
    }
}

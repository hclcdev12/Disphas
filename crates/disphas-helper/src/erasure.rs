//! Erasure primitives: targets, sample selection, canaries, host overwrite
//! and validation (`docs/moteur_decision.md` §5).

use chacha20::ChaCha20;
use chacha20::cipher::{KeyIvInit, StreamCipher, StreamCipherSeek};
use disphas_core::method::Method;
use disphas_core::outcome::Validation;
use disphas_core::profile::AtaSmart;
use disphas_device::block::BlockIo;
use sha2::{Digest, Sha256};

/// A drive opened for erasure, after every check passed.
pub trait ErasureTarget: Send {
    /// Block access to the drive.
    fn io(&mut self) -> &mut dyn BlockIo;

    /// Runs a firmware method and returns whether the firmware reports success.
    /// `progress` receives percentages.
    fn firmware(&mut self, method: Method, progress: &mut dyn FnMut(u8)) -> Result<bool, String>;

    /// The drive's own estimate of `method`'s duration, in seconds, for a
    /// firmware method that reports no progress while it runs (ATA Security
    /// Erase). Shown to the operator only, never certified.
    fn firmware_estimate(&mut self, _method: Method) -> Option<u64> {
        None
    }

    /// Waits for a firmware Sanitize started by a helper that crashed, and
    /// returns whether the firmware reports success. Refuses by default.
    fn resume_firmware(
        &mut self,
        _method: Method,
        _progress: &mut dyn FnMut(u8),
    ) -> Result<bool, String> {
        Err("this drive cannot resume an erasure".to_owned())
    }

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

/// The pseudo-random stream written by a host overwrite: ChaCha20 (RFC 8439)
/// keyed by SHA-256(seed ‖ "overwrite"), independent of canaries and samples.
/// Byte `n` of the drive receives byte `n` of the stream, so the content
/// expected in any block can be recomputed for validation. This is the
/// "Datenstrom aus Zufallswerten (PRNG Stream)" of BSI IT-Grundschutz
/// CON.6.A12 (`docs/conformite.md`).
fn overwrite_stream(seed: &[u8; 32]) -> ChaCha20 {
    let key: [u8; 32] = Sha256::new()
        .chain_update(seed)
        .chain_update(b"overwrite")
        .finalize()
        .into();
    ChaCha20::new(&key.into(), &[0u8; 12].into())
}

/// Content that block `lba` holds after a host overwrite with `seed`.
pub fn overwrite_block(seed: &[u8; 32], lba: u64, block_size: u64) -> Vec<u8> {
    let mut block = vec![0u8; block_size as usize];
    let mut stream = overwrite_stream(seed);
    stream.seek(lba * block_size);
    stream.apply_keystream(&mut block);
    block
}

/// Overwrites every block with the pseudo-random stream of `seed`, then
/// makes it durable.
pub fn host_overwrite(
    io: &mut dyn BlockIo,
    seed: &[u8; 32],
    progress: &mut dyn FnMut(HostProgress),
) -> std::io::Result<()> {
    let block_size = io.block_size();
    let total = io.block_count();
    let blocks_per_chunk = (CHUNK_BYTES / block_size).max(1);
    let mut stream = overwrite_stream(seed);
    let mut chunk = vec![0u8; (blocks_per_chunk * block_size) as usize];
    let mut lba = 0;
    while lba < total {
        let count = blocks_per_chunk.min(total - lba);
        let data = &mut chunk[..(count * block_size) as usize];
        // The stream runs in disk order: chunk n follows chunk n-1.
        data.fill(0);
        stream.apply_keystream(data);
        io.write_blocks(lba, data)?;
        lba += count;
        progress(HostProgress {
            bytes_done: lba * block_size,
            bytes_total: total * block_size,
        });
    }
    io.sync()
}

/// Overwrites every block with zeros, then makes it durable: the optional
/// final pass after the pseudo-random one, for a drive that looks blank.
pub fn zero_fill(
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
    /// Every sample reads back as zeros (after the final zero pass).
    Zeros,
    /// Every sample holds the overwrite stream expected there (host overwrite).
    Pattern,
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
            Check::Pattern => buffer != overwrite_block(seed, lba, block_size),
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
    use disphas_device::block::FileImage;

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
    fn host_overwrite_then_pattern_validation_passes() {
        let (_dir, mut image) = image(20_000);
        let seed = [3; 32];
        let lbas = samples(&seed, image.block_count(), 100);
        write_canaries(&mut image, &seed, &lbas).expect("canaries");
        assert_eq!(
            validate(&mut image, &seed, &lbas, Check::Pattern),
            Validation::Mismatch
        );
        let mut last = None;
        host_overwrite(&mut image, &seed, &mut |progress| last = Some(progress))
            .expect("overwrite");
        assert_eq!(last.map(|p| p.bytes_done), Some(20_000 * 512));
        assert_eq!(
            validate(&mut image, &seed, &lbas, Check::Pattern),
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
        host_overwrite(&mut image, &seed, &mut |_| {}).expect("overwrite");
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
    fn overwrite_stream_is_random_looking_and_position_bound() {
        let (_dir, mut image) = image(64);
        let seed = [5; 32];
        host_overwrite(&mut image, &seed, &mut |_| {}).expect("overwrite");
        let mut block = vec![0u8; 512];
        image.read_blocks(10, &mut block).expect("read");
        assert_eq!(
            block,
            overwrite_block(&seed, 10, 512),
            "recomputable per block"
        );
        assert_ne!(
            block,
            overwrite_block(&seed, 11, 512),
            "differs per position"
        );
        assert_ne!(
            block,
            overwrite_block(&[6; 32], 10, 512),
            "differs per seed"
        );
        // Not a constant pattern: about half of the bits are set.
        let ones: u32 = block.iter().map(|byte| byte.count_ones()).sum();
        assert!((1800..2300).contains(&ones), "{ones} bits set out of 4096");
        // A zeroed block, as a lazy or lying writer would leave, fails.
        image.write_blocks(10, &[0; 512]).expect("write");
        assert_eq!(
            validate(&mut image, &seed, &[10], Check::Pattern),
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
            validate(&mut Broken, &[0; 32], &[0, 5], Check::Pattern),
            Validation::ReadErrors
        );
    }
}

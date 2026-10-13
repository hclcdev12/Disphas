//! Block access used by the erasure engine: read and write whole logical
//! blocks, and make writes durable.
//!
//! [`FileImage`] works on disk image files only: it refuses, by construction,
//! anything that is not a regular file, so it can never open a device node.
//! Real block devices will get their own implementation (O_DIRECT, aligned
//! buffers) behind the same trait.

use std::fs::{File, OpenOptions};
use std::io;
use std::os::unix::fs::FileExt;
use std::path::Path;

/// Block-level read and write access to a drive or an image.
pub trait BlockIo: Send {
    /// Logical block size in bytes.
    fn block_size(&self) -> u64;

    /// Number of logical blocks.
    fn block_count(&self) -> u64;

    /// Reads `buffer.len()` bytes starting at block `lba`.
    fn read_blocks(&mut self, lba: u64, buffer: &mut [u8]) -> io::Result<()>;

    /// Writes `data` starting at block `lba`. `data` holds whole blocks.
    fn write_blocks(&mut self, lba: u64, data: &[u8]) -> io::Result<()>;

    /// Makes every write durable.
    fn sync(&mut self) -> io::Result<()>;

    /// Size in bytes.
    fn size_bytes(&self) -> u64 {
        self.block_count().saturating_mul(self.block_size())
    }
}

/// A disk image file, for tests and demonstrations.
#[derive(Debug)]
pub struct FileImage {
    file: File,
    block_size: u64,
    block_count: u64,
}

impl FileImage {
    /// Opens an existing image file read-write. Refuses anything that is not
    /// a regular file (block or character devices, directories, sockets…),
    /// and images whose size is not a whole number of blocks.
    pub fn open(path: &Path, block_size: u64) -> io::Result<Self> {
        let metadata = std::fs::symlink_metadata(path)?;
        if !metadata.file_type().is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a regular file: FileImage never opens devices",
            ));
        }
        if block_size == 0 || metadata.len() % block_size != 0 || metadata.len() == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "image size is not a whole number of blocks",
            ));
        }
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        Ok(Self {
            file,
            block_size,
            block_count: metadata.len() / block_size,
        })
    }

    /// Creates a sparse image of `blocks` blocks at `path`, unless it exists.
    pub fn create(path: &Path, block_size: u64, blocks: u64) -> io::Result<Self> {
        if !path.exists() {
            let file = OpenOptions::new().write(true).create_new(true).open(path)?;
            file.set_len(block_size.saturating_mul(blocks))?;
        }
        Self::open(path, block_size)
    }

    fn check(&self, lba: u64, len: usize) -> io::Result<u64> {
        let len = len as u64;
        let fits = len % self.block_size == 0
            && lba
                .checked_add(len / self.block_size)
                .is_some_and(|end| end <= self.block_count);
        if !fits {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "access outside the image or not block-aligned",
            ));
        }
        Ok(lba * self.block_size)
    }
}

impl BlockIo for FileImage {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn block_count(&self) -> u64 {
        self.block_count
    }

    fn read_blocks(&mut self, lba: u64, buffer: &mut [u8]) -> io::Result<()> {
        let offset = self.check(lba, buffer.len())?;
        self.file.read_exact_at(buffer, offset)
    }

    fn write_blocks(&mut self, lba: u64, data: &[u8]) -> io::Result<()> {
        let offset = self.check(lba, data.len())?;
        self.file.write_all_at(data, offset)
    }

    fn sync(&mut self) -> io::Result<()> {
        self.file.sync_all()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_and_reads_blocks() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let mut image = FileImage::create(&dir.path().join("disk.img"), 512, 16).expect("created");
        assert_eq!(image.size_bytes(), 8192);
        image.write_blocks(3, &[0xab; 1024]).expect("written");
        let mut buffer = [0u8; 512];
        image.read_blocks(4, &mut buffer).expect("read");
        assert_eq!(buffer, [0xab; 512]);
        image.read_blocks(0, &mut buffer).expect("read");
        assert_eq!(buffer, [0; 512], "sparse image reads as zeros");
    }

    #[test]
    fn refuses_access_outside_or_unaligned() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let mut image = FileImage::create(&dir.path().join("disk.img"), 512, 4).expect("created");
        assert!(image.write_blocks(4, &[0; 512]).is_err());
        assert!(image.write_blocks(3, &[0; 1024]).is_err());
        assert!(image.write_blocks(0, &[0; 100]).is_err());
        assert!(image.write_blocks(u64::MAX, &[0; 512]).is_err());
    }

    #[test]
    fn never_opens_anything_but_a_regular_file() {
        for path in ["/dev/null", "/dev/zero", "/tmp"] {
            assert!(FileImage::open(Path::new(path), 512).is_err(), "{path}");
        }
    }

    #[test]
    fn refuses_symlinks_to_devices() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let link = dir.path().join("evil.img");
        std::os::unix::fs::symlink("/dev/zero", &link).expect("symlink");
        assert!(FileImage::open(&link, 512).is_err());
    }
}

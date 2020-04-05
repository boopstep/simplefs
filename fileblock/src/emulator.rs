use crate::blockio::{BlockNumber, BlockStorage};
use std::fs::{File, OpenOptions};
use std::io::prelude::*;
use std::io::{ErrorKind, SeekFrom};
use std::path::PathBuf;

/// 4k is a common block size for file systems. Disks commonly are composed of
/// 512 byte blocks mapping each file system block to 8 hard disk blocks.
static BLOCK_SIZE_BYTES: usize = 4096;

struct FileBlockEmulator {
    /// The file must be a fixed-size file some exact multiple of the size of a block.
    fd: File,
    /// The total number of blocks available in the file store.
    block_count: usize,
}

/// Emulates block disk/flash storage in userspace using a file as block storage.
/// This is only meant to be used for file system development and testing.
impl FileBlockEmulator {
    /// Returns ownership of the underlying file descriptor to the caller.
    pub fn into_file(self) -> File {
        self.fd
    }
}

impl BlockStorage for FileBlockEmulator {
    fn open_disk(dest: &PathBuf, nblocks: usize) -> std::io::Result<Self>
    where
        Self: std::marker::Sized,
    {
        // Do not create if the file does not exist.
        let file = OpenOptions::new().write(true).create_new(true).open(dest)?;
        let emu = FileBlockEmulator {
            fd: file,
            block_count: nblocks,
        };

        Ok(emu)
    }

    fn read_block(&mut self, blocknr: BlockNumber, buf: &mut [u8]) -> std::io::Result<()> {
        if blocknr > (self.block_count - 1) {
            return Err(std::io::Error::new(
                ErrorKind::InvalidInput,
                "block requested exceeds filesystem upper bound",
            ));
        }

        if buf.len() < BLOCK_SIZE_BYTES {
            return Err(std::io::Error::new(
                ErrorKind::InvalidInput,
                "buffer does not contain enough space to read block",
            ));
        }
        // FIXME(allancalix): Keep a seek pointer in file descriptor to avoid
        // having to seek from start each time.
        self.fd
            .seek(SeekFrom::Start((blocknr * BLOCK_SIZE_BYTES) as u64))?;

        // IO reads enough bytes to fill the buffer it receives. In order to limit
        // the number of bytes to one block we allocate a fixed sized buffer to fill.
        let mut fixed_block = vec![0; BLOCK_SIZE_BYTES];
        let bytes_read = self.fd.read(fixed_block.as_mut_slice())?;
        debug_assert!(bytes_read == BLOCK_SIZE_BYTES);

        buf.copy_from_slice(&fixed_block);
        Ok(())
    }

    fn write_block(&mut self, blocknr: BlockNumber, buf: &mut [u8]) -> std::io::Result<()> {
        if blocknr > (self.block_count - 1) {
            return Err(std::io::Error::new(
                ErrorKind::InvalidInput,
                "block requested exceeds filesystem upper bound",
            ));
        }

        if buf.len() < BLOCK_SIZE_BYTES {
            return Err(std::io::Error::new(
                ErrorKind::InvalidInput,
                "buffer does not contain enough space to read block",
            ));
        }
        self.fd
            .seek(SeekFrom::Start((blocknr * BLOCK_SIZE_BYTES) as u64))?;

        let mut fixed_block = vec![0x00; BLOCK_SIZE_BYTES];
        fixed_block.copy_from_slice(buf);
        let bytes_written = self.fd.write(fixed_block.as_mut_slice())?;
        debug_assert!(bytes_written == BLOCK_SIZE_BYTES);
        Ok(())
    }

    fn sync_disk(&mut self) -> std::io::Result<()> {
        self.fd.sync_all()?;
        Ok(())
    }
}

struct FileBlockEmulatorBuilder {
    fd: File,
    block_count: usize,
}

impl From<File> for FileBlockEmulatorBuilder {
    fn from(fd: File) -> Self {
        FileBlockEmulatorBuilder {
            fd,
            // A better default here might be the size of the file rounded down
            // to the nearest block.
            block_count: 0,
        }
    }
}

impl FileBlockEmulatorBuilder {
    /// Sets the number of desired blocks in the block store device.
    pub fn with_block_size(mut self, blocks: usize) -> Self {
        self.block_count = blocks;
        self
    }

    /// This builder assumed ownership of the file descriptor used and does
    /// destructive things to prepare the file for use. Additionally, ownership
    /// of the file is transfered to the emulator meaning this builder can only
    /// be used to create one emulator.
    pub fn build(mut self) -> std::io::Result<FileBlockEmulator> {
        debug_assert!(self.block_count > 0);
        self.zero_block()?;
        Ok(FileBlockEmulator {
            fd: self.fd,
            block_count: self.block_count,
        })
    }

    fn zero_block(&mut self) -> std::io::Result<()> {
        let total_bytes = self.block_count * BLOCK_SIZE_BYTES;
        let bytes_written = self
            .fd
            // FIXME(allancalix): Clean up heap allocation.
            .write(vec![0x00; total_bytes].as_slice())?;
        debug_assert!(bytes_written == self.block_count * BLOCK_SIZE_BYTES);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn file_emulator_allocates_correct_num_bytes() {
        let fs_block = tempfile::tempfile().unwrap();
        let mut disk_emu = FileBlockEmulatorBuilder::from(fs_block)
            .with_block_size(4)
            .build()
            .expect("failed to allocate file block");
        // let mut disk_emu =
        //     FileBlockEmulator::from(fs_block, 4).expect("failed to allocate file block");
        disk_emu.sync_disk().unwrap();
        assert_eq!(disk_emu.into_file().metadata().unwrap().len(), 4 * 4096);
    }

    #[test]
    fn can_read_and_write_blocks() {
        let fs_block = tempfile::tempfile().unwrap();
        // let mut disk_emu =
        //     FileBlockEmulator::from(fs_block, 4).expect("failed to allocate file block");
        let mut disk_emu = FileBlockEmulatorBuilder::from(fs_block)
            .with_block_size(4)
            .build()
            .expect("failed to allocate file block");
        disk_emu.sync_disk().unwrap();

        // Allocate a block with a non-zero character.
        let mut block = vec![0x55; 4096];
        disk_emu.write_block(2, block.as_mut_slice()).unwrap();
        disk_emu.sync_disk().unwrap();

        let mut read_block = vec![0x00; 4096];
        // Read a different block.
        disk_emu.read_block(3, read_block.as_mut_slice()).unwrap();
        assert_eq!(read_block, vec![0x00; 4096]);

        // Read the block with data.
        let mut filled_block = vec![0x00; 4096];
        disk_emu.read_block(2, filled_block.as_mut_slice()).unwrap();
        assert_eq!(filled_block, vec![0x55; 4096]);
    }

    #[test]
    fn can_read_and_write_start_and_end_blocks() {
        let fs_block = tempfile::tempfile().unwrap();
        // let mut disk_emu =
        //     FileBlockEmulator::from(fs_block, 4).expect("failed to allocate file block");
        let mut disk_emu = FileBlockEmulatorBuilder::from(fs_block)
            .with_block_size(2)
            .build()
            .expect("failed to allocate file block");
        disk_emu.sync_disk().unwrap();

        let mut block = vec![0x55; 4096];
        disk_emu.write_block(0, block.as_mut_slice()).unwrap();
        disk_emu.sync_disk().unwrap();

        let mut read_block = vec![0x00; 4096];
        disk_emu.read_block(0, read_block.as_mut_slice()).unwrap();
        assert_eq!(read_block, vec![0x55; 4096]);

        // Allocate a block with a non-zero character.
        let mut block = vec![0x55; 4096];
        disk_emu.write_block(1, block.as_mut_slice()).unwrap();
        disk_emu.sync_disk().unwrap();

        let mut read_block = vec![0x00; 4096];
        // Read a different block.
        disk_emu.read_block(1, read_block.as_mut_slice()).unwrap();
        assert_eq!(read_block, vec![0x55; 4096]);
    }

    #[test]
    fn read_block_beyond_range_throws_exception() {
        let fs_block = tempfile::tempfile().unwrap();
        // let mut disk_emu =
        //     FileBlockEmulator::from(fs_block, 4).expect("failed to allocate file block");
        let mut disk_emu = FileBlockEmulatorBuilder::from(fs_block)
            .with_block_size(1)
            .build()
            .expect("failed to allocate file block");
        disk_emu.sync_disk().unwrap();

        // Attempt to write beyond range.
        let mut block = vec![0x55; 4096];
        let wresult = disk_emu.write_block(1, block.as_mut_slice());
        match wresult {
            Ok(_) => assert!(false, "expected an error"),
            Err(_) => (),
        }
    }
}

use std::path::Path;

use crate::alloc::{Bitmap, State};
use crate::io::BlockStorage;
use crate::node::InodeGroup;
use crate::sb::SuperBlock;

use std::collections::HashMap;
use std::ffi::OsString;
use thiserror::Error;

const SB_MAGIC: u32 = 0x5346_5342; // SFSB

pub const BLOCK_SIZE: usize = 4096;
const NODE_SIZE: usize = 256;

/// Known locations.
const SUPERBLOCK_INDEX: usize = 0;
const DATA_REGION_BMP: usize = 1;
const INODE_BMP: usize = 2;
const INODE_START: usize = 3;

impl Default for SuperBlock {
    fn default() -> Self {
        let mut sb = SuperBlock::new();
        sb.sb_magic = SB_MAGIC;
        // This is a limited implementation only supporting at most 80 file system
        // objects (files or directories).
        sb.inodes_count = 5 * (BLOCK_SIZE / NODE_SIZE) as u32;
        // Use the remaining space for user data blocks.
        sb.blocks_count = 56;
        sb.reserved_blocks_count = 0;
        sb.free_blocks_count = 0;
        // All inodes are initially free.
        sb.free_inodes_count = sb.inodes_count;
        sb
    }
}

// Encodes open filesystem call options http://man7.org/linux/man-pages/man2/open.2.html.
pub enum OpenMode {
    RO,
    WO,
    RW,
    DIRECTORY,
    CREATE,
}

#[derive(Error, Debug)]
pub enum SFSError {
    #[error("found no file at path")]
    DoesNotExist,
    #[error("invalid file system block layout")]
    InvalidBlock(#[from] std::io::Error),
}
/// A fixed 64 4k block file system. Currently hard coded for simplicity with
/// one super block, one inode bitmap, one data block bitmap, five inode blocks,
/// and 56 blocks for data storage.
pub struct SFS<T: BlockStorage> {
    dev: T,
    super_block: SuperBlock,
    data_map: Bitmap,
    inodes: InodeGroup,
}

impl<T: BlockStorage> SFS<T> {
    /// Initializes the file system onto owned block storage.
    ///
    /// # Layout
    /// ==============================================================================
    /// | SuperBlock | Bitmap (data region) | Bitmap (inodes) | Inodes | Data Region |
    /// ==============================================================================
    pub fn create(mut dev: T) -> Result<Self, SFSError> {
        // Reusable buffer for writing blocks.
        let mut block_buffer = [0; 4096];

        // Init SuperBlock header.
        let super_block = SuperBlock::default();
        &block_buffer[0..28].copy_from_slice(super_block.serialize());
        dev.write_block(SUPERBLOCK_INDEX, &mut block_buffer)?;

        // Init allocation map for data region.
        let data_map = Bitmap::new();
        &block_buffer.copy_from_slice(data_map.serialize());
        dev.write_block(DATA_REGION_BMP, &mut block_buffer)?;

        // Initialize inode structure with root node.
        let inodes = InodeGroup::new(Bitmap::new());
        &block_buffer.copy_from_slice(inodes.allocations().serialize());
        dev.write_block(INODE_BMP, &mut block_buffer)?;
        dev.write_block(INODE_START, &mut inodes.serialize_block(0))?;
        dev.sync_disk()?;

        Ok(SFS {
            dev,
            inodes,
            data_map,
            super_block,
        })
    }

    pub fn open(mut dev: T) -> Result<Self, SFSError> {
        let mut block_buf = vec![0; 4096];

        // Read superblock from first block;
        dev.read_block(SUPERBLOCK_INDEX, &mut block_buf)?;
        let super_block = SuperBlock::parse(&block_buf, SB_MAGIC);

        dev.read_block(DATA_REGION_BMP, &mut block_buf)?;
        let data_map = Bitmap::parse(&block_buf);

        dev.read_block(INODE_BMP, &mut block_buf)?;
        let inode_allocs = Bitmap::parse(&block_buf);
        let mut inodes = InodeGroup::open(inode_allocs);

        for i in INODE_START..INODE_START + 5 {
            dev.read_block(i, &mut block_buf)?;
            // TODO(allancalix): This is a bit ugly. Because the inode group is unaware that's first
            // disk block is at an offset (INODE_START) we have to subtract the offset before loading
            // the block.
            inodes.load_block((i - INODE_START) as u32, &block_buf);
        }

        Ok(SFS {
            dev,
            inodes,
            data_map,
            super_block,
        })
    }

    /// Opens a file descriptor at the path provided. By default, this implementation will return an
    /// error if the file does not exists. Set OpenMode to override the behavior and create a file or
    /// directory.
    pub fn open_file<P: AsRef<Path>>(&mut self, path: P, mode: OpenMode) -> Result<u32, SFSError> {
        let mut parts = path.as_ref().components();
        if Some(std::path::Component::RootDir) != parts.next() {
            // TODO(allancalix): Throw a different error here, something invalid argument-y.
            return Err(SFSError::DoesNotExist);
        }

        let inum = 0;
        for part in parts {
            let mut content = self.read_dir(inum)?;
            if content.get(part.as_os_str()).is_none() {
                match mode {
                    OpenMode::CREATE => {
                        // A few things need to happen here.
                        // 1. A new inode should be allocated and added to the map.
                        // 2. The new inumber and part name should be written to the current
                        //    directories content.
                        let created_file = self.inodes.new_file();
                        content.insert(OsString::from(part.as_os_str()), created_file);
                        self.write_dir(inum, content)?;
                        return Ok(created_file);
                    },
                    _ => {
                        return Err(SFSError::DoesNotExist);
                    },
                }
            }

            unimplemented!()
        }
        Ok(inum)
    }

    // TODO(allancalix): Create real allocation policy.
    fn next_free_data_block(&self) -> usize {
        for block in 0..56 {
            if let State::Free = self.data_map.get(block as usize) {
                return block;
            }
        }
        panic!("No data blocks left to allocate");
    }

    // TODO(allancalix): Still need to mark reserved nodes as such and update the inode with new
    // blocks belonging to it.
    fn write_dir(&mut self, dir: u32, entries: HashMap<OsString, u32>) -> Result<(), SFSError> {
        let mut contents: String = entries
            .iter()
            .map(|(k, v)| format!("{}:{}\n", v, k.to_str().unwrap()))
            .collect();
        contents.push('\0');

        let node = self.inodes.get(dir).unwrap();
        let allocated_blocks: Vec<u32> = node
            .blocks
            .iter()
            .filter(|block| *block > &8_u32)
            .map(|u| *u)
            .collect();
        if allocated_blocks.len() < 1 + (contents.as_bytes().len() / BLOCK_SIZE) {
            // TODO(allancalix): We'll have to allocate more blocks here, a problem for another day.
            let needed = 1 + (contents.as_bytes().len() / BLOCK_SIZE);
            let have = allocated_blocks.len();

            // TODO(allancalix): WARNING - THERE IS A BUG HERE. Next free data block is a placeholder
            // for an actual allocation policy. The current implementation returns the same values on
            // repeated calls.
            let new_blocks: Vec<u32> = (0..(needed - have)).map(|_| self.next_free_data_block() as u32).collect();
            let mut all_blocks = allocated_blocks.iter().chain(new_blocks.iter());

            unsafe {
                contents.as_bytes_mut().chunks_mut(BLOCK_SIZE).for_each(|chunk| {
                    self.dev.write_block(*all_blocks.next().unwrap() as usize, chunk).unwrap();
                });
            }
            return Ok(())
        }

        println!("Writing content \"{}\" to dir inode {}.", contents, dir);
        let mut blocks = allocated_blocks.iter();
        unsafe {
            contents.as_bytes_mut().chunks_mut(BLOCK_SIZE).for_each(|chunk| {
                self.dev.write_block(*blocks.next().unwrap() as usize, chunk).unwrap();
            });
        }
        Ok(())
    }

    fn read_dir(&mut self, inum: u32) -> Result<HashMap<OsString, u32>, SFSError> {
        let content = self.read_file(inum)?;
        let contents_parsed = String::from_utf8(content).unwrap();

        let mut dir_contents = HashMap::new();
        for line in contents_parsed.lines() {
            let mut contents = line.split(':');
            let entry_inum = contents.next().unwrap().parse::<u32>().unwrap();
            let entry_name = OsString::from(contents.next().unwrap());
            dir_contents.insert(entry_name, entry_inum);
        }

        Ok(dir_contents)
    }

    fn read_file(&mut self, inum: u32) -> Result<Vec<u8>, SFSError> {
        let node = self.inodes.get(inum);
        if node.is_none() {
            return Err(SFSError::DoesNotExist);
        }
        let allocated_blocks: Vec<u32> = node
            .unwrap()
            .blocks
            .iter()
            .filter(|block| *block > &8_u32)
            .map(|u| *u)
            .collect();

        let mut content = vec![0; allocated_blocks.len()];
        for (i, &block) in allocated_blocks.iter().enumerate() {
            let start = i * BLOCK_SIZE;
            let end = start + BLOCK_SIZE;
            self.dev
                .read_block(block as usize, &mut content[start..end])?;
        }
        Ok(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::{FileBlockEmulator, FileBlockEmulatorBuilder};

    fn create_test_device() -> FileBlockEmulator {
        let dev = tempfile::tempfile().unwrap();
        FileBlockEmulatorBuilder::from(dev)
            .with_block_size(64)
            .build()
            .expect("Could not initialize disk emulator.")
    }

    #[test]
    fn root_dir_returns_root_fd() {
        let dev = create_test_device();
        let mut fs = SFS::create(dev).unwrap();
        assert_eq!(fs.open_file("/", OpenMode::RO).unwrap(), 0);
    }

    #[test]
    fn file_not_found_without_create_returns_error() {
        let dev = create_test_device();
        let mut fs = SFS::create(dev).unwrap();

        let result = fs.open_file("/foo", OpenMode::RO);
        match result.unwrap_err() {
            SFSError::DoesNotExist => (),
            _ => assert!(false, "Unexpected error type."),
        }
    }

    #[test]
    fn file_not_found_with_create_returns_handle() {
        let dev = create_test_device();

        let mut fs = SFS::create(dev).unwrap();

        assert_eq!(fs.open_file("/foo", OpenMode::CREATE).unwrap(), 1);
    }

    #[test]
    fn can_create_and_reopen_initialized_filesystem() {
        let disk = tempfile::NamedTempFile::new().unwrap();
        let dev = FileBlockEmulatorBuilder::from(disk.reopen().unwrap())
            .with_block_size(64)
            .build()
            .unwrap();
        // Initialize the filesystem.
        SFS::create(dev).unwrap();

        let dev = FileBlockEmulatorBuilder::from(disk.reopen().unwrap())
            .with_block_size(64)
            // Don't reset initialized disk.
            .clear_medium(false)
            .build()
            .unwrap();
        let fs: SFS<FileBlockEmulator> = SFS::open(dev).unwrap();
        assert_eq!(fs.inodes.total_nodes(), 1);
    }
}

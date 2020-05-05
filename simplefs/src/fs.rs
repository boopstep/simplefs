use std::path::Path;

use crate::alloc::Bitmap;
use crate::io::BlockStorage;
use crate::node::InodeGroup;
use crate::sb::SuperBlock;

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

    pub fn open(mut dev: T, blocknr: usize) -> Result<Self, SFSError> {
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
    pub fn open_file<P: AsRef<Path>>(&mut self, path: P, _mode: OpenMode) -> Result<u32, SFSError> {
        let mut parts = path.as_ref().components();
        if Some(std::path::Component::RootDir) != parts.next() {
            // TODO(allancalix): Throw a different error here, something invalid argument-y.
            return Err(SFSError::DoesNotExist);
        }

        let mut inum = 0;
        for part in parts {
            let inode = self.inodes.get(inum).unwrap();

            unimplemented!();
            // let content = self.read_inode(root);
            // let next_node_index = search(content, part);
            // if let None = next_node_idnex {
            //   return Err(SFSError::DoesNotExist);
            // }
        }
        Ok(inum)
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
        let fs: SFS<FileBlockEmulator> = SFS::open(dev, 64).unwrap();
        assert_eq!(fs.inodes.total_nodes(), 1);
    }

    // #[test]
    // fn file_not_found_with_create_returns_handle() {
    //       let dev = create_test_device();
    //
    //       let fs = SFS::create(dev).unwrap();
    //       assert_eq!(fs.open_file("/foo", OpenMode::CREATE).unwrap(), 1);
    //   }
    //
    //   #[test]
    //   #[should_panic]
    //   fn inodes_not_including_data_return_none() {
    //       let dev = create_test_device();
    //
    //       let fs = SFS::create(dev).unwrap();
    //       fs.open_file("/foo/bar", OpenMode::RO).unwrap();
    //   }
}

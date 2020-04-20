use std::fs::File;
use std::path::Path;
use thiserror::Error;

use crate::emulator::{FileBlockEmulator, FileBlockEmulatorBuilder};
use crate::alloc::BitmapBlock;
use crate::blockio::BlockStorage;
use crate::sb::SuperBlock;

const SB_MAGIC: u32 = 0x5346_5342; // SFSB

#[derive(Error, Debug)]
pub enum SFSError {
  #[error("invalid file system block layout")]
  InvalidBlock(#[from] std::io::Error),
}
/// A fixed 64 4k block file system. Currently hard coded for simplicity with
/// one super block, one inode bitmap, one data block bitmap, five inode blocks,
/// and 56 blocks for data storage.
pub struct SFS<T: BlockStorage> {
    dev: T,
    super_block: SuperBlock,
    inode_bmp: BitmapBlock,
    data_bmp: BitmapBlock,
    // TODO(allancalix): inode structure.
}

impl SFS<FileBlockEmulator> {
    /// Initializes the file system onto owned block storage.
    pub fn create(dev: File, blocks: usize) -> Result<Self, SFSError> {
        let mut emu = FileBlockEmulatorBuilder::from(dev)
            .with_block_size(blocks)
            .build()
            .unwrap();
        let sb = SFS::prepare_sb();
        let inode_bmp = BitmapBlock::new();
        let data_bmp = BitmapBlock::new();

        emu.write_block(0, &mut sb.serialize())?;
        emu.write_block(1, &mut data_bmp.serialize()[0..4096])?;
        emu.write_block(2, &mut inode_bmp.serialize()[0..4096])?;

        Ok(SFS {
            dev: emu,
            super_block: sb,
            inode_bmp: BitmapBlock::new(),
            data_bmp: BitmapBlock::new(),
        })
    }

    pub fn open<P: AsRef<Path>>(disk: P, blocknr: usize) -> Result<Self, SFSError> {
        let mut emu = FileBlockEmulator::open_disk(&disk, blocknr)?;
        let mut block_buf = vec![0; 4096];

        // Read superblock from first block;
        emu.read_block(0, &mut block_buf)?;
        let super_block = SuperBlock::parse(&block_buf, SB_MAGIC);

        // Read inode bitmap from second block;
        emu.read_block(1, &mut block_buf)?;
        let data_bmp = BitmapBlock::from(&block_buf);

        emu.read_block(2, &mut block_buf)?;
        let inode_bmp = BitmapBlock::from(&block_buf);

        Ok(Self {
            dev: emu,
            super_block,
            inode_bmp,
            data_bmp,
        })
    }

    fn prepare_sb() -> SuperBlock {
        let mut sb = SuperBlock::new();
        sb.sb_magic = SB_MAGIC;
        // This is a limited implementation only supporting at most 80 file system
        // objects (files or directories).
        sb.inodes_count = 5 * (4096 / 256);
        // Use the remaining space for user data blocks.
        sb.blocks_count = 56;
        sb.reserved_blocks_count = 0;
        sb.free_blocks_count = 0;
        // All inodes are initially free.
        sb.free_inodes_count = sb.inodes_count;
        sb
    }
}

impl From<File> for SFS<FileBlockEmulator> {
    fn from(fd: File) -> Self {
        let mut emu = FileBlockEmulator{
          fd,
          block_count: 64,
        };
        let mut block_buf = vec![0; 4096];

        // Read superblock from first block;
        emu.read_block(0, &mut block_buf).unwrap();
        let super_block = SuperBlock::parse(&block_buf, SB_MAGIC);

        // Read inode bitmap from second block;
        emu.read_block(1, &mut block_buf).unwrap();
        let data_bmp = BitmapBlock::from(&block_buf);

        emu.read_block(2, &mut block_buf).unwrap();
        let inode_bmp = BitmapBlock::from(&block_buf);

        Self {
            dev: emu,
            super_block,
            inode_bmp,
            data_bmp,
        }
    }
}

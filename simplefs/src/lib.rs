mod alloc;
mod blockio;
pub mod emulator;
mod sb;

use crate::alloc::BitmapBlock;
use crate::blockio::BlockStorage;
use crate::sb::SuperBlock;
use std::fs::File;

const SB_MAGIC: u32 = 0x53465342; // SFSB

/// A fixed 64 4k block file system. Currently hard coded for simplicity with
/// one super block, one inode bitmap, one data block bitmap, five inode blocks,
/// and 56 blocks for data storage.
pub struct SFS<T: BlockStorage> {
    dev: T,
    super_block: SuperBlock,
    inode_bmp: alloc::BitmapBlock,
    data_bmp: alloc::BitmapBlock,
}

impl SFS<emulator::FileBlockEmulator> {
    /// Initializes the file system onto owned block storage.
    pub fn create(dev: File, blocks: usize) -> Result<Self, std::io::Error> {
        let mut emu = emulator::FileBlockEmulatorBuilder::from(dev)
            .with_block_size(blocks)
            .build()
            .unwrap();
        let sb = SFS::prepare_sb();
        let inode_bmp = BitmapBlock::new();
        let data_bmp = BitmapBlock::new();

        emu.write_block(0, &mut sb.serialize())?;
        emu.write_block(1, &mut inode_bmp.serialize()[0..4096])?;
        emu.write_block(2, &mut data_bmp.serialize()[0..4096])?;

        Ok(SFS {
            dev: emu,
            super_block: sb,
            inode_bmp: BitmapBlock::new(),
            data_bmp: BitmapBlock::new(),
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
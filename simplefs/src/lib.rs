mod alloc;
mod blockio;
mod sb;
pub mod emulator;

use std::fs::File;
use crate::sb::SuperBlock;
use crate::blockio::BlockStorage;

pub struct SFS<T: BlockStorage> {
    dev: T,
    super_block: SuperBlock,
    // inode_bmp: alloc::BitmapBlock,
    // data_bmp: alloc::BitmapBlock,
    // TODO(allancalix): inode ds
}

impl SFS<emulator::FileBlockEmulator> {
    pub fn create(dev: File, blocks: usize) -> Result<(), std::io::Error> {
        let mut emu = emulator::FileBlockEmulatorBuilder::from(dev)
            .with_block_size(blocks)
            .build().unwrap();
        let sb = SuperBlock::new();
        emu.write_block(0, &mut sb.serialize())
    }
}

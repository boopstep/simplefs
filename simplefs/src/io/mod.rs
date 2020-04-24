mod diskemu;
mod block;

pub(crate) use block::BlockStorage;
pub(crate) use diskemu::{FileBlockEmulator, FileBlockEmulatorBuilder};

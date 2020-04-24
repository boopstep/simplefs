mod block;
mod diskemu;

pub(crate) use block::BlockStorage;
pub use diskemu::{FileBlockEmulator, FileBlockEmulatorBuilder};

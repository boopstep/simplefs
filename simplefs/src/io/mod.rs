mod block;
mod file;

pub(crate) use block::BlockStorage;
pub use file::{FileBlockEmulator, FileBlockEmulatorBuilder};

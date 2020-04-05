use std::path::PathBuf;

pub type BlockNumber = usize;

/// Tried to map as closely as possible to the prescribed interface found here:
/// http://web.mit.edu/6.033/1997/handouts/html/04sfs.html.
///
/// In cases where implementing the interface as described would lead to non-idiomatic
/// rust code, I opted to use a more rust-y interface.
pub trait BlockStorage {
    fn open_disk(path: &PathBuf, nblocks: usize) -> std::io::Result<Self>
    where
        Self: std::marker::Sized;

    fn read_block(&mut self, blocknr: BlockNumber, buf: &mut [u8]) -> std::io::Result<()>;

    fn write_block(&mut self, blocknr: BlockNumber, buf: &mut [u8]) -> std::io::Result<()>;

    fn sync_disk(&mut self) -> std::io::Result<()>;
}

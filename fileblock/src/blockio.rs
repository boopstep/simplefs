use std::path::Path;

/// The block number to access ranging from 0 (the first block) to n - 1 (the last
/// block) where n is number of blocks available.
pub type BlockNumber = usize;

/// Tried to map as closely as possible to the prescribed interface found here:
/// http://web.mit.edu/6.033/1997/handouts/html/04sfs.html.
///
/// In cases where implementing the interface as described would lead to non-idiomatic
/// rust code, I opted to use a more rust-y interface.
pub trait BlockStorage {
    /// Opens a disk at the specified path. This method does not validate the
    /// storage blocks, it is up for clients to ensure disks are appropriately initialized.
    fn open_disk<P: AsRef<Path>>(path: P, nblocks: usize) -> std::io::Result<Self>
    where
        Self: std::marker::Sized;
    /// Reads disk block number into provided buffer.
    ///
    /// # Errors
    ///
    /// Attempting to read a block out of range will return an error.
    fn read_block(&mut self, blocknr: BlockNumber, buf: &mut [u8]) -> std::io::Result<()>;
    /// Writes provided buffer into the specified block number. Attempting to write
    /// a block out of range will return an error.
    /// Writes provided buffer into the specified block number.
    ///
    /// # Errors
    ///
    /// Attempting to write a block out of range will return an error.
    fn write_block(&mut self, blocknr: BlockNumber, buf: &mut [u8]) -> std::io::Result<()>;
    /// Flush any buffered disk IO from memory. This is useful if it must guaranteed
    /// the disk writes actually occurred, for instance, if being re-read from
    /// disk.
    fn sync_disk(&mut self) -> std::io::Result<()>;
}

mod blockio;
mod alloc;
pub mod emulator;

/// The first block of the file system storing information critical for mounting
/// the file system and verifying the underlying disk is formatted correctly.
///
/// Keeps the size of the file system by tracking the number of blocks allocated
/// to the inode and data block groups. The number of nodes available in the filesystem
/// ultimately sets the upper bound on how many files can exist.
///
/// Some files, such as files with no data and symbolic links don't allocate any
/// data blocks but do allocate inode blocks.
struct SuperBlock {
    /// A 32-bit identifying string, in this case SFSB.
    sb_magic: u32,
    /// Assuming 256 bytes per inode a 4K block can hold 16 inodes.
    inodes_count: u32,
    /// All the remaining blocks are allocating to storing user data.
    blocks_count: u32,
    /// All blocks currently in use by the filesystem.
    reserved_blocks_count: u32,
    /// All blocks available to be allocated by the system.
    free_blocks_count: u32,
    /// The number of remaining available inodes.
    free_inodes_count: u32,
}

impl SuperBlock {
    fn new() -> Self {
        Self {
            sb_magic: 0x53465342, // SFSB
            inodes_count: 0,
            blocks_count: 0,
            reserved_blocks_count: 0,
            free_blocks_count: 0,
            free_inodes_count: 0,
        }
    }
}

pub fn init() {
}

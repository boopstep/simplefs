use zerocopy::{FromBytes, AsBytes};

/// The first block of the file system storing information critical for mounting
/// the file system and verifying the underlying disk is formatted correctly.
///
/// Keeps the size of the file system by tracking the number of blocks allocated
/// to the inode and data block groups. The number of nodes available in the filesystem
/// ultimately sets the upper bound on how many files can exist.
///
/// Some files, such as files with no data and symbolic links don't allocate any
/// data blocks but do allocate inode blocks.
#[repr(C)]
#[derive(Debug, PartialEq, AsBytes, FromBytes, Clone, Copy)]
pub struct SuperBlock {
    /// A 32-bit identifying string, in this case SFSB.
    pub sb_magic: u32,
    /// Assuming 256 bytes per inode a 4K block can hold 16 inodes.
    pub inodes_count: u32,
    /// All the remaining blocks are allocating to storing user data.
    pub blocks_count: u32,
    /// All blocks currently in use by the filesystem.
    pub reserved_blocks_count: u32,
    /// All blocks available to be allocated by the system.
    pub free_blocks_count: u32,
    /// The number of remaining available inodes.
    pub free_inodes_count: u32,
    /// The index of the next available free block.
    pub free_list: u32,
}

impl SuperBlock {
    pub fn new() -> Self {
        Self {
            sb_magic: 0, // Default to invalid zero value.
            inodes_count: 0,
            blocks_count: 0,
            reserved_blocks_count: 0,
            free_blocks_count: 0,
            free_inodes_count: 0,
            free_list: 0,
        }
    }

    /// Attempts to parse a buffer as a SuperBlock returning a new owned instance
    /// of the block. If the block is invalid, calling parse will cause a panic.
    pub fn parse(buf: &[u8], magic: u32) -> Self {
        let sb: *const SuperBlock = buf.as_ptr() as *const SuperBlock;

        unsafe {
            assert_eq!(
                magic, (*sb).sb_magic,
                "Superblock magic constant invalid."
            );
            SuperBlock::from(*sb)
        }
    }

    /// Serializes the superblock into a series of bytes that can be sent or
    /// deserialized back into a SuperBlock;
    pub fn serialize<'a>(&'a self) -> &'a [u8] {
      self.as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_MAGIC: u32 = 0x4EEE; // non-zero superblock value.

    #[test]
    fn can_encode_and_decode_superblocks() {
        let mut sb = SuperBlock::new();
        sb.sb_magic = TEST_MAGIC; // non-zero superblock value.
        sb.inodes_count = 5;
        sb.blocks_count = 56;
        let encoded = sb.serialize();

        let parsed = SuperBlock::parse(encoded, TEST_MAGIC);

        assert_eq!(parsed, sb);
    }

    #[test]
    #[should_panic(expected = "Superblock magic constant invalid.")]
    fn parsing_buffer_with_invalid_magic_panics() {
        let zero_buffer_with_right_size = vec![0; 4096];
        SuperBlock::parse(&zero_buffer_with_right_size, TEST_MAGIC);
    }
}

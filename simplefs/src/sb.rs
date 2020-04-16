use std::convert::TryInto;

const BLOCK_SIZE: usize = 4096;

const SB_MAGIC: u32 = 0x53465342;

/// The first block of the file system storing information critical for mounting
/// the file system and verifying the underlying disk is formatted correctly.
///
/// Keeps the size of the file system by tracking the number of blocks allocated
/// to the inode and data block groups. The number of nodes available in the filesystem
/// ultimately sets the upper bound on how many files can exist.
///
/// Some files, such as files with no data and symbolic links don't allocate any
/// data blocks but do allocate inode blocks.
#[derive(Debug, PartialEq)]
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
}

impl SuperBlock {
    pub fn new() -> Self {
        Self {
            sb_magic: SB_MAGIC, // SFSB
            inodes_count: 0,
            blocks_count: 0,
            reserved_blocks_count: 0,
            free_blocks_count: 0,
            free_inodes_count: 0,
        }
    }

    /// Reads a the super block from a buffer of of exactly size BLOCK_SIZE. Passing
    /// a slice of any other size will result in a panic.
    pub fn parse(buf: &[u8]) -> Self {
        assert_eq!(buf.len(), BLOCK_SIZE, "Length of buffer to parse must equal block size.");
        let mut sb = Self::new();

        let read_magic = u32::from_be_bytes(buf[0..4].try_into().unwrap());
        assert_eq!(read_magic, sb.sb_magic, "Superblock magic constant invalid.");

        sb.inodes_count = u32::from_be_bytes(buf[4..8].try_into().unwrap());
        sb.blocks_count = u32::from_be_bytes(buf[8..12].try_into().unwrap());
        sb.reserved_blocks_count = u32::from_be_bytes(buf[12..16].try_into().unwrap());
        sb.free_blocks_count = u32::from_be_bytes(buf[16..20].try_into().unwrap());
        sb.free_inodes_count = u32::from_be_bytes(buf[20..24].try_into().unwrap());
        return sb
    }

    /// Serializes the SuperBlock into a BLOCK_SIZE buffer for writing to disk.
    /// The encoding is a series of struct fields with big endian alignment.
    pub fn serialize(&self) -> Vec<u8> {
        let mut sb_encoded = vec!();
        sb_encoded.extend_from_slice(&self.sb_magic.to_be_bytes());
        sb_encoded.extend_from_slice(&self.inodes_count.to_be_bytes());
        sb_encoded.extend_from_slice(&self.blocks_count.to_be_bytes());
        sb_encoded.extend_from_slice(&self.reserved_blocks_count.to_be_bytes());
        sb_encoded.extend_from_slice(&self.free_blocks_count.to_be_bytes());
        sb_encoded.extend_from_slice(&self.free_inodes_count.to_be_bytes());
        // FIXME(allancalix): This is lazy coding to try and append bytes to the
        // end of this buffer to fill a fixed space.
        sb_encoded.extend_from_slice(&[0;4072]);
        sb_encoded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_encode_and_decode_superblocks() {
        let mut sb = SuperBlock::new();
        sb.inodes_count = 5;
        sb.blocks_count = 56;
        let encoded = sb.serialize();

        let parsed = SuperBlock::parse(&encoded[0..4096]);

        assert_eq!(parsed, sb);
    }

    #[test]
    #[should_panic(expected = "Superblock magic constant invalid.")]
    fn parsing_buffer_with_invalid_magic_panics() {
        let zero_buffer_with_right_size = vec![0;4096];
        SuperBlock::parse(&zero_buffer_with_right_size);
    }

    #[test]
    #[should_panic]
    fn parsing_buffer_with_invalid_size_panics() {
        let wrong_size_buffer = vec![0;512];
        SuperBlock::parse(&wrong_size_buffer);
    }
}

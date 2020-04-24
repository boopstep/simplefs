use std::collections::BTreeMap;
use std::path::Path;

use crate::alloc::Bitmap;
use crate::io::BlockStorage;
use crate::sb::SuperBlock;

use thiserror::Error;
use zerocopy::{AsBytes, FromBytes};

pub const BLOCK_SIZE: usize = 4096;

const SB_MAGIC: u32 = 0x5346_5342; // SFSB

const NODE_SIZE: usize = 256;

const FT_UNKNOWN: u8 = 0;
const FT_FILE: u8 = 1;
const FT_DIR: u8 = 2;

#[repr(packed)]
#[derive(AsBytes, FromBytes)]
pub struct Inode {
    /// The absolute postition of the inode in the filesystem.
    inumber: u32,
    /// The file mode (e.g full access - drwxrwxrwx).
    mode: u16,
    /// The total size in bytes.
    size: u32,
    /// The id of the owning user.
    uid: u16,
    /// The id of the owning group.
    gid: u16,
    /// Pointers for the data blocks that belong to the file. Uses the remaining
    /// space the 256 inode space.
    blocks: [u32; (NODE_SIZE - 96) / 32],
    // TODO(allancalix): Fill in the rest of the metadata like access time, create
    // time, modification time, symlink information.
}

impl Inode {
    fn default() -> Self {
        Self {
            inumber: 0,
            mode: 0,
            size: 0,
            uid: 0,
            gid: 0,
            blocks: [0; (NODE_SIZE - 96) / 32],
        }
    }
}

#[derive(Error, Debug)]
pub enum SFSError {
    #[error("invalid file system block layout")]
    InvalidBlock(#[from] std::io::Error),
}
/// A fixed 64 4k block file system. Currently hard coded for simplicity with
/// one super block, one inode bitmap, one data block bitmap, five inode blocks,
/// and 56 blocks for data storage.
pub struct SFS<T: BlockStorage> {
    dev: T,
    super_block: SuperBlock,
    data_map: Bitmap,
    inode_map: Bitmap,
    inodes: BTreeMap<u32, Inode>,
    // TODO(allancalix): inode structure.
}

impl<T: BlockStorage> SFS<T> {
    /// Initializes the file system onto owned block storage.
    pub fn create(mut dev: T) -> Result<Self, SFSError> {
        let sb = SFS::<T>::prepare_sb();

        let mut block_buffer = [0; 4096];
        &block_buffer[0..28].copy_from_slice(sb.serialize());
        dev.write_block(0, &mut block_buffer)?;

        let data_map = Bitmap::new();
        &block_buffer.copy_from_slice(data_map.serialize());
        dev.write_block(1, &mut block_buffer)?;

        let root = create_root_node();
        let mut inodes = BTreeMap::new();
        &block_buffer[0..256].copy_from_slice(root.as_bytes());
        inodes.insert(0, root);
        dev.write_block(3, &mut block_buffer)?;

        // Create inode allocation tracker and set root block to reserved.
        let mut inode_map = Bitmap::new();
        inode_map.set_reserved(0);
        &block_buffer.copy_from_slice(inode_map.serialize());
        dev.write_block(2, &mut block_buffer)?;

        Ok(SFS {
            dev,
            data_map,
            inode_map,
            super_block: sb,
            inodes,
        })
    }

    pub fn open<P: AsRef<Path>>(disk: P, blocknr: usize) -> Result<Self, SFSError> {
        let mut dev = T::open_disk(&disk, blocknr)?;
        let mut block_buf = vec![0; 4096];

        // Read superblock from first block;
        dev.read_block(0, &mut block_buf)?;
        let super_block = SuperBlock::parse(&block_buf, SB_MAGIC);

        dev.read_block(1, &mut block_buf)?;
        let data_map = Bitmap::parse(&block_buf);

        dev.read_block(3, &mut block_buf)?;
        let inode_map = Bitmap::parse(&block_buf);

        Ok(SFS {
            dev,
            data_map,
            inode_map,
            super_block,
            inodes: BTreeMap::new(),
        })
    }

    fn prepare_sb() -> SuperBlock {
        let mut sb = SuperBlock::new();
        sb.sb_magic = SB_MAGIC;
        // This is a limited implementation only supporting at most 80 file system
        // objects (files or directories).
        sb.inodes_count = 5 * (BLOCK_SIZE / NODE_SIZE) as u32;
        // Use the remaining space for user data blocks.
        sb.blocks_count = 56;
        sb.reserved_blocks_count = 0;
        sb.free_blocks_count = 0;
        // All inodes are initially free.
        sb.free_inodes_count = sb.inodes_count;
        sb
    }
}

fn create_root_node() -> Inode {
    let mut root = Inode::default();
    // TODO(allancalix): Set real root value here.
    root.mode = 0x4000;

    root
}

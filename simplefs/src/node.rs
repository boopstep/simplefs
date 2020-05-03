use std::collections::{BTreeMap, HashSet};

use crate::alloc::{Bitmap, State};

use crate::io::BlockStorage;
use zerocopy::{AsBytes, FromBytes};

const BLOCK_SIZE: u32 = 4096;
const NODE_SIZE: u32 = 256;
const NODES_PER_BLOCK: u32 = BLOCK_SIZE / NODE_SIZE;
const ROOT_DEFAULT_MODE: u16 = 0x4000;

#[repr(C)]
#[derive(AsBytes, FromBytes, Copy, Clone)]
/// This structure __must not exceed 256 bytes.__
pub struct Inode {
    /// The file mode (e.g full access - drwxrwxrwx).
    mode: u16,
    /// The id of the owning user.
    uid: u16,
    /// The id of the owning group.
    gid: u16,
    /// The number of links to this file.
    links_count: u16,
    /// The total size of the file in bytes.
    size: u32,
    /// The time the file was created in milliseconds since epoch.
    create_time: u32,
    /// The time the file was last updated in milliseconds since epoch.
    update_time: u32,
    /// The time the file was last accessed in milliseconds since epoch.
    access_time: u32,
    /// Reserved for future expansion of file attributes up to 256 byte limit.
    // TODO(allancalix): Fill in the rest of the metadata like  symlink information etc.
    padding: [u32; 43],
    /// Pointers for the data blocks that belong to the file. Uses the remaining
    /// space the 256 inode space.
    blocks: [u32; 15],
}

enum InodeStatus {
    /// The entity requested exists.
    Found(u32),
    /// The parent handle if traversal finds parent directory but not terminal entity.
    NotFound(u32),
}

impl Inode {
    fn root() -> Self {
        Self {
            mode: ROOT_DEFAULT_MODE,
            uid: 0,
            gid: 0,
            links_count: 0,
            size: 0,
            create_time: 0,
            update_time: 0,
            access_time: 0,
            padding: [0; 43],
            blocks: [0; 15],
        }
    }

    fn default() -> Self {
        Self {
            // TODO(allancalix): Probably find another mode.
            mode: ROOT_DEFAULT_MODE,
            uid: 0,
            gid: 0,
            links_count: 0,
            size: 0,
            create_time: 0,
            update_time: 0,
            access_time: 0,
            padding: [0; 43],
            blocks: [0; 15],
        }
    }

    fn parse(buf: &[u8]) -> Self {
        let inode = buf.clone().as_ptr() as *const Inode;
        unsafe { *inode }
    }
}

pub struct InodeGroup<T: BlockStorage> {
    store: T,
    nodes: BTreeMap<u32, Inode>,
    alloc_tracker: Bitmap,
}

impl<T: BlockStorage> InodeGroup<T> {
    pub fn new(store: T, alloc_tracker: Bitmap) -> Self {
        let mut inodes = BTreeMap::new();
        let mut group = Self {
            store,
            nodes: inodes,
            alloc_tracker,
        };

        group.insert(0, Inode::root());
        group
    }

    pub fn open(store: T, alloc_tracker: Bitmap, disk_blocks: u32) -> Self {
        let mut allocated_blocks = HashSet::new();
        for i in 0..(disk_blocks * NODES_PER_BLOCK) {
            if let State::Used = alloc_tracker.get(i as usize) {
                allocated_blocks.insert(i / NODES_PER_BLOCK);
            }
        }

        let mut group = Self {
            store,
            alloc_tracker,
            nodes: BTreeMap::new(),
        };
        for block in allocated_blocks.into_iter() {
            println!("Loading inodes from block {}.", block);
            group.load_block(block).unwrap();
        }

        println!("Inodes loaded from disk.");
        group
    }

    pub fn total_nodes(&self) -> usize {
        self.nodes.len()
    }

    pub fn insert(&mut self, node_block: u32, node: Inode) {
        // TODO(allancalix): Allocation tracker needs sync on insert.
        self.alloc_tracker.set_reserved(node_block as usize);
        self.nodes.insert(node_block, node);
        let disk_block = self.get_disk_block(node_block);
        self.store
            .write_block(disk_block, &mut self.serialize_block(disk_block as u32))
            .unwrap();
    }

    fn get_disk_block(&self, node_block: u32) -> usize {
        (node_block / NODES_PER_BLOCK) as usize
    }

    /// Loads a disk block of inodes into the in-memory tree.
    fn load_block(&mut self, disk_block: u32) -> Result<(), std::io::Error> {
        let offset = disk_block * NODES_PER_BLOCK;
        let mut block_buf = vec![0; 4096];
        for i in offset..NODES_PER_BLOCK {
            if let State::Used = self.alloc_tracker.get(i as usize) {
                self.store.read_block(disk_block as usize, &mut block_buf)?;
                let node_offset = offset as usize % NODE_SIZE as usize;
                let node = Inode::parse(&block_buf[node_offset..NODE_SIZE as usize]);
                self.nodes.insert(i, node);
            }
        }
        Ok(())
    }

    /// Serializes an entire disk block of inodes for writing to disk.
    fn serialize_block(&self, disk_block: u32) -> Vec<u8> {
        let mut block_buf = vec![0; 4096];
        let offset = disk_block * NODES_PER_BLOCK;
        let mut buffer_index = 0;
        for (_, node) in self.nodes.range(offset..NODES_PER_BLOCK) {
            &block_buf[buffer_index..NODE_SIZE as usize].copy_from_slice(node.as_bytes());
            buffer_index += NODE_SIZE as usize;
        }

        block_buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alloc::Bitmap;
    use crate::io::FileBlockEmulatorBuilder;

    #[test]
    fn can_serialize_and_deserialize_inode() {
        let mut root = Inode::root();
        // Change some values.
        root.uid = 100;
        root.gid = 100;

        let parsed_root = Inode::parse(root.clone().as_bytes());

        assert_eq!(root.uid, parsed_root.uid);
        assert_eq!(root.gid, parsed_root.gid);
    }

    #[test]
    fn can_serialize_entire_block_to_buffer() {
        let fd = tempfile::tempfile().unwrap();
        let dev = FileBlockEmulatorBuilder::from(fd.try_clone().unwrap())
            .with_block_size(64)
            .build()
            .expect("Could not initialize disk emulator.");

        let nodes_map = Bitmap::new();
        InodeGroup::new(dev, nodes_map);

        let dev = FileBlockEmulatorBuilder::from(fd)
            .with_block_size(64)
            .build()
            .expect("Could not initialize disk emulator.");
        let mut nodes_map = Bitmap::new();
        nodes_map.set_reserved(0); // root node
        let group = InodeGroup::open(dev, nodes_map, 5);
        assert_eq!(group.total_nodes(), 1);
    }
}

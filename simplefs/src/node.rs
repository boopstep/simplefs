use std::collections::{BTreeMap, HashSet};

use crate::alloc::{Bitmap, State};

use crate::io::BlockStorage;
use zerocopy::{AsBytes, FromBytes};

const BLOCK_SIZE: u32 = 4096;
const NODE_SIZE: u32 = 256;
const NODES_PER_BLOCK: u32 = BLOCK_SIZE / NODE_SIZE;
const ROOT_DEFAULT_MODE: u16 = 0x4000;
const DEFAULT_MODE: u16 = 0x2000;

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
            mode: DEFAULT_MODE,
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
            group.load_block(block).unwrap();
        }

        group
    }

    pub fn total_nodes(&self) -> usize {
        self.nodes.len()
    }

    fn insert(&mut self, node_block: u32, node: Inode) -> Result<(), std::io::Error> {
        // TODO(allancalix): Allocation tracker needs sync on insert.
        self.alloc_tracker.set_reserved(node_block as usize);
        self.nodes.insert(node_block, node);
        let disk_block = self.get_disk_block(node_block);
        self.store
            .write_block(disk_block, &mut self.serialize_block(disk_block as u32))?;
        self.store.sync_disk()?;
        Ok(())
    }

    fn get_disk_block(&self, node_block: u32) -> usize {
        (node_block / NODES_PER_BLOCK) as usize
    }

    /// Loads a disk block of inodes into the in-memory tree.
    fn load_block(&mut self, disk_block: u32) -> Result<(), std::io::Error> {
        let block_start = disk_block * NODES_PER_BLOCK;
        let block_end = block_start + NODES_PER_BLOCK;
        let mut block_buf = vec![0; 4096];
        for i in block_start..block_end {
            if let State::Used = self.alloc_tracker.get(i as usize) {
                self.store.read_block(disk_block as usize, &mut block_buf)?;
                let node_offset = block_start as usize % NODE_SIZE as usize;
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
        for (i, node) in self.nodes.range(offset..NODES_PER_BLOCK) {
            let node_offset = *i as usize * NODE_SIZE as usize;
            &block_buf[node_offset..node_offset + NODE_SIZE as usize]
                .copy_from_slice(node.as_bytes());
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
        let mut init_group = InodeGroup::new(dev, nodes_map);
        init_group.insert(1, Inode::default());
        // Add an inode to a second disk block.
        init_group.insert(16, Inode::default());
        assert_eq!(init_group.total_nodes(), 3);

        let dev = FileBlockEmulatorBuilder::from(fd)
            .with_block_size(64)
            .build()
            .expect("Could not initialize disk emulator.");
        let mut nodes_map = Bitmap::new();
        nodes_map.set_reserved(0); // root node initialized by default.
        nodes_map.set_reserved(1);
        nodes_map.set_reserved(16);
        let group = InodeGroup::open(dev, nodes_map, 5);

        assert_eq!(group.total_nodes(), 3);
        assert_eq!(group.nodes.get(&0).unwrap().mode, ROOT_DEFAULT_MODE);
        assert!(group.nodes.get(&1).is_some());
        assert!(group.nodes.get(&16).is_some());
    }
}

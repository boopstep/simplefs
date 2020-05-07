use std::collections::BTreeMap;

use crate::alloc::{Bitmap, State};

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
    pub blocks: [u32; 15],
}

enum _InodeStatus {
    /// The entity requested exists.
    _Found(u32),
    /// The parent handle if traversal finds parent directory but not terminal entity.
    _NotFound(u32),
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
        let inode = buf.as_ptr() as *const Inode;
        unsafe { *inode }
    }
}

pub struct InodeGroup {
    nodes: BTreeMap<u32, Inode>,
    alloc_tracker: Bitmap,
}

impl InodeGroup {
    pub fn new(alloc_tracker: Bitmap) -> Self {
        let mut group = Self {
            nodes: BTreeMap::new(),
            alloc_tracker,
        };

        group.insert(0, Inode::root());
        group
    }

    pub fn open(alloc_tracker: Bitmap) -> Self {
        Self {
            nodes: BTreeMap::new(),
            alloc_tracker,
        }
    }

    pub fn get(&self, inum: u32) -> Option<&Inode> {
        self.nodes.get(&inum)
    }

    pub fn allocations(&self) -> &Bitmap {
        &self.alloc_tracker
    }

    #[allow(dead_code)] // Will need this at some point.
    pub fn total_nodes(&self) -> usize {
        self.nodes.len()
    }

    /// Allocates a regular file Inode into the table and returns the new reserved node allocation
    /// block index (i.e. the inumber). Panics if there is no space left to allocate another node.
    pub fn new_file(&mut self) -> u32 {
        for block in 0..NODES_PER_BLOCK * 5 {
            if let State::Free = self.alloc_tracker.get(block as usize) {
                let new_node = Inode::default();
                self.insert(block, new_node);
                return block;
            }
        }
        panic!("No free space left to allocate nodes.")
    }

    fn insert(&mut self, node_block: u32, node: Inode) -> usize {
        // TODO(allancalix): Allocation tracker needs write to disk on insert.
        self.alloc_tracker.set_reserved(node_block as usize);
        self.nodes.insert(node_block, node);
        self.get_disk_block(node_block)
    }

    fn get_disk_block(&self, node_block: u32) -> usize {
        (node_block / NODES_PER_BLOCK) as usize
    }

    /// Loads a disk block of inodes into the in-memory tree.
    pub fn load_block(&mut self, disk_block: u32, block_buf: &[u8]) {
        let block_start = disk_block * NODES_PER_BLOCK;
        let block_end = block_start + NODES_PER_BLOCK;
        for i in block_start..block_end {
            if let State::Used = self.alloc_tracker.get(i as usize) {
                let node_offset = block_start as usize % NODE_SIZE as usize;
                let node = Inode::parse(&block_buf[node_offset..NODE_SIZE as usize]);
                self.nodes.insert(i, node);
            }
        }
    }

    /// Serializes an entire disk block of inodes for writing to disk.
    pub fn serialize_block(&self, disk_block: u32) -> Vec<u8> {
        let mut block_buf = vec![0; 4096];
        let offset = disk_block * NODES_PER_BLOCK;
        for (i, node) in self.nodes.range(offset..NODES_PER_BLOCK) {
            let node_offset = *i as usize * NODE_SIZE as usize;
            block_buf[node_offset..node_offset + NODE_SIZE as usize]
                .copy_from_slice(node.as_bytes());
        }

        block_buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alloc::Bitmap;

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
    fn can_retrieve_inserted_inode() {
        let nodes_map = Bitmap::new();
        let mut group = InodeGroup::new(nodes_map);
        let mut node = Inode::default();
        node.uid = 100;
        node.gid = 100;
        group.insert(1, node);

        assert_eq!(group.get(1).unwrap().uid, 100);
        assert_eq!(group.get(1).unwrap().gid, 100);
    }
}

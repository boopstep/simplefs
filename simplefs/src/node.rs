use std::collections::BTreeMap;

use crate::alloc::Bitmap;

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
}

pub struct InodeGroup {
  nodes: BTreeMap<u32, Inode>,
  alloc_tracker: Bitmap,
}

impl InodeGroup {
  pub fn new(alloc_tracker: Bitmap) -> Self {
    let mut inodes = BTreeMap::new();
    let mut group = Self {
      nodes: inodes,
      alloc_tracker,
    };

    group.insert(0, Inode::root());
    group
  }

  pub fn insert(&mut self, node_block: u32, node: Inode) {
    // TODO(allancalix): Needs to sync to disk.
    self.alloc_tracker.set_reserved(node_block as usize);
    self.nodes.insert(node_block, node);
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

  #[test]
  fn can_serialize_entire_block_to_buffer() {
    let nodes_map = Bitmap::new();
    let mut group = InodeGroup::new(nodes_map);
    assert_eq!(group.serialize_block(1), vec![0; 4096]);
  }
}


// slightly arbitrary size of our "disk": 4GIB, broken into 4KiB blocks
// 4 * 2.pow(30) / 4096 == 2Kib required to store bitmap
const BITMAP_LEN: usize = 1048576_usize;

/// disk block free list 
/// persistent tracking of what blocks have been allocated.
/// option 1: Bitmap! naive style
struct  DiskBlockFreelist {
    // keep an array of bits indicating whether a file
    bits: [usize; BITMAP_LEN]
}

/// option 2: B-Tree!!
/// wip...
struct DiskBlockFreeTree {
    count: u32,
}

struct Node {
    val: u32,
    // don't know how to imlement this yet without a Box; is it even possible?
    // children: [DiskBlockFreeTree; BITMAP_LEN]
}




/// an description of the type of file object pointed to by an Inode
enum FileType {
    /// a regular file
    RegularFile,
    /// a directory tree containing 1 or more regular files or directories
    Directory
}

/// index node containing most of the interesting bits about a file object on disk
struct Inode {
    inumber: u32,
    ftype: FileType,
    fsize: u64,
    block_location_id: u32,
    uid: u16 // owner
}

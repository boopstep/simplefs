/// fuse
use std::os::raw::{c_char, c_int};

#[repr(C)]
#[derive(Debug)]
pub struct fuse_args {
    pub argc: c_int,
    pub argv: *const *const c_char,
    pub allocated: c_int
}

#[repr(C)]
#[derive(Debug)]
pub struct fuse_operations {
    //  there are like 2 dozen more to add...
	pub readlink: *const *const c_char
}

extern "C" {
    pub fn fuse_mount_compat25(mountpoint: *const c_char, args: *const fuse_args) -> c_int;
    pub fn fuse_main(args: *const fuse_args, op: *const fuse_operations, private_data: *const c_char) -> c_int;
}

use simplefs_fuse;
use std::env;

pub fn main() {
    let args: Vec<String> = env::args().collect();
    let l = args.len() as u8;

    simplefs_fuse::mount();
}

use tempfile;

use simplefs::{self, SFS};

pub fn main() {
    let tmp = tempfile::tempfile().unwrap();
    let dev = simplefs::io::FileBlockEmulatorBuilder::from(tmp)
        .with_block_size(64)
        .build()
        .expect("Could not initialize disk emulator.");

    // create a new simple fs on device and open /
    let mut sfs = SFS::create(dev).expect("should create");
    sfs.open("/", simplefs::OpenMode::RO).unwrap();
}

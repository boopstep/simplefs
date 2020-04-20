use tempfile::NamedTempFile;
use simplefs::SFS;

#[test]
fn can_initialize_disk_with_filesystem() {
  let tf = NamedTempFile::new().unwrap();

  // Prepare the block with filesystem layout.
  SFS::create(tf.reopen().unwrap(), 64).unwrap();

  // Open filesystem and verify init layout;
  SFS::from(tf.into_file());
}

#[test]
#[should_panic]
fn unformatted_blocks_panic() {
  let tf = NamedTempFile::new().unwrap();
  // Open filesystem and verify init layout;
  SFS::from(tf.into_file());
}

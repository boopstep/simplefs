extern crate pkg_config;

pub fn main() {
   // link to the fuse lib 
   println!("cargo:rustc-link-lib=dylib=fuse");

   pkg_config::Config::new().atleast_version("2.9.6").probe("fuse").unwrap();
}

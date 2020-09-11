use voxfs_tool_lib::Handler;
use voxfs::DiskHandler;

fn main() {
    let path = "test.voximg".to_string();
    let size = 4000;

    let handler = Handler::new(path).unwrap();
    println!("{:?}", handler.read_bytes(0x140, 6).unwrap());
}

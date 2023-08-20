use static_include_bytes::static_include_bytes;

static_include_bytes!(DCD, concat!(env!("OUT_DIR"), "/dcd.bin"));

fn main() {
    println!("{}", DCD.len());
    for chunk in DCD.chunks(16) {
        for &byte in chunk {
            print!("{:02X} ", byte);
        }
        println!();
    }
}

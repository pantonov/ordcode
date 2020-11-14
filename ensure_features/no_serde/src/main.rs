extern crate biord;
fn main() {
    let mut buf = [0u8; 8];
    let mut w = biord::DeBytesWriter::new(&mut buf);
    biord::primitives::serialize_u32(&mut w, 1234, biord::params::DescendingOrder).unwrap();

    println!("Hello, {:#?}!", buf);
}

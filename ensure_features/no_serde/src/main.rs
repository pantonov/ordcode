extern crate ordcode;
fn main() {
    let mut buf = [0u8; 8];
    let mut w = ordcode::DeBytesWriter::new(&mut buf);
    ordcode::primitives::serialize_u32(&mut w, 1234, ordcode::params::DescendingOrder).unwrap();

    println!("Hello, {:#?}!", buf);
}

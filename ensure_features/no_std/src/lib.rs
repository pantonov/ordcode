#![no_std]

extern crate biord;
extern crate serde;
#[macro_use] extern crate serde_derive;

#[derive(Serialize, Deserialize)]
pub struct MyStuff {
    x:  u16,
    y:  [f32; 10],
}

pub fn serialize_my(v: &MyStuff, to: &mut [u8]) -> biord::Result<usize> {
    biord::ser_to_buf_ordered(v, to, biord::Order::Ascending)
}

pub fn deserialize_my(buf: &[u8]) -> biord::Result<MyStuff> {
    biord::de_from_bytes_ordered_asc(buf)
}

use crate::{ ReadBytes, WriteBytes, Result, ResultExt, ErrorKind,
             Order, BytesBuf, BytesBufExt };


pub fn serialize_bytes(writer: &mut impl WriteBytes, value: &[u8], order: Order) -> Result
{

}

pub fn deserialize_bytes_to_writer(reader: &mut impl ReadBytes, out: &mut impl WriteBytes, order: Order) -> Result
{

}

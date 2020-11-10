use biord::{ *, varint::*  };

// Varint tests are adopted and modified from VInt implementation, github.com/iqlusioninc/veriform
// Original Copyright © 2017-2020 Tony Arcieri
// Original license: https://www.apache.org/licenses/LICENSE-2.0
fn encode(value: u64) -> Box<[u8]> {
    let mut s = Vec::<u8>::new();
    varu64_encode_to_writer(&mut s, value).unwrap();
    s.into_boxed_slice()
}

fn decode(bytes: &[u8]) -> Result<(u64, u8)> {
    varu64_decode_from_slice(bytes)
}

#[test]
fn encode_zero() {
    assert_eq!(encode(0).as_ref(), &[1]);
}

#[test]
fn encode_bit_pattern_examples() {
    assert_eq!(encode(0x0f0f).as_ref(), &[0x3e, 0x3c]);
    assert_eq!(encode(0x0f0f_f0f0).as_ref(), &[0x08, 0x0f, 0xff, 0xf0]);
    assert_eq!(
        encode(0x0f0f_f0f0_0f0f).as_ref(),
        &[0xc0, 0x87, 0x07, 0x78, 0xf8, 0x87, 0x07]
    );
    assert_eq!(
        encode(0x0f0f_f0f0_0f0f_f0f0).as_ref(),
        &[0x00, 0xf0, 0xf0, 0x0f, 0x0f, 0xf0, 0xf0, 0x0f, 0x0f]
    );
}

#[test]
fn encode_maxint() {
    assert_eq!(
        encode(core::u64::MAX).as_ref(),
        &[0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]
    );
}

#[test]
fn decode_zero() {
    let slice = [1].as_ref();
    assert_eq!(decode(&slice).unwrap(), (0, 1));
}

#[test]
fn decode_bit_pattern_examples() {
    let slice = [0x3e, 0x3c].as_ref();
    assert_eq!(decode(&slice).unwrap(), (0x0f0f, 2));

    let slice = [0x08, 0x0f, 0xff, 0xf0].as_ref();
    assert_eq!(decode(&slice).unwrap(), (0x0f0f_f0f0, 4));

    let slice = [0xc0, 0x87, 0x07, 0x78, 0xf8, 0x87, 0x07].as_ref();
    assert_eq!(decode(&slice).unwrap(), (0x0f0f_f0f0_0f0f, 7));

    let slice = [0x00, 0xf0, 0xf0, 0x0f, 0x0f, 0xf0, 0xf0, 0x0f, 0x0f].as_ref();
    assert_eq!(decode(&slice).unwrap(), (0x0f0f_f0f0_0f0f_f0f0, 9));
}

#[test]
fn decode_maxint() {
    let slice = [0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff].as_ref();
    assert_eq!(decode(slice).unwrap(), (core::u64::MAX, 9));
}

#[test]
fn decode_with_trailing_data() {
    let slice = [0x3e, 0x3c, 0xde, 0xad, 0xbe, 0xef].as_ref();
    assert_eq!(decode(slice).unwrap(), (0x0f0f, 2));
}

#[test]
fn decode_truncated() {
    let slice = [0].as_ref();
    assert!(decode(slice).is_err());

    let slice = [0x08, 0x0f, 0xff].as_ref();
    assert!(decode(slice).is_err());
}

#[cfg(debug_assertions)]
#[test]
fn decode_trailing_zeroes() {
    let slice = [0x08, 0x00, 0x00, 0x00].as_ref();
    assert!(decode(slice).is_err());
}
use std::fmt::Display;

// construct error object
macro_rules! errobj {
    ($i:ident) => { crate::Error::from_kind(crate::errors::ErrorKind::$i) }
}

// construct Result::Err(errobj)
macro_rules! err {
    ($i:ident) => { Err(errobj!($i)) }
}

error_chain! {
    errors {
        Serde(t: String) { description("serde error"), display("serde error: {}", t) }
        SerializeSequenceMustHaveLength { description("serialize: sequence must have length") }
        BytesReadAllNotImplemented { description("BytesRead::apply_all not implemented") }
        BufferOverflow { description("BytesWrite:: Buffer overflow") }
        BufferUnderflow { description("BytesWrite:: Buffer underflow") }
        PrematureEndOfInput { description("premature end of input data") }
        InvalidByteSequenceEscape { description("bad escape byte value in serialized byte sequence") }
        DeserializeAnyNotSupported { description("deserialize_any not supported") }
        DeserializeIdentifierNotSupported { description("deserialize_identifier not supported") }
        DeserializeIgnoredAny { description("ignored_any not supported") }
        InvalidUtf8Encoding { description("invalid UTF-8 encoding in deserialized string") }
        InvalidTagEncoding { description("invalid enum tag value") }
        InvalidVarintEncoding { description("invalid varint byte sequence") }
    }
}

#[cfg(feature="serde")]
impl serde::ser::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        ErrorKind::Serde(msg.to_string()).into()
    }
}

#[cfg(feature="serde")]
impl serde::de::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        ErrorKind::Serde(msg.to_string()).into()
    }
}

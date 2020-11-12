/// Serialization errors
#[allow(clippy::manual_non_exhaustive)]
#[derive(Debug, Copy, Clone)]
pub enum Error {
    #[doc(hidden)]
    _Serde, // not used, but need to satisfy serde Error traits
    SerializeSequenceMustHaveLength,
    BufferOverflow,
    BufferUnderflow,
    PrematureEndOfInput,
    InvalidByteSequenceEscape,
    DeserializeAnyNotSupported,
    DeserializeIdentifierNotSupported,
    DeserializeIgnoredAny,
    InvalidUtf8Encoding,
    InvalidTagEncoding,
    InvalidVarintEncoding,
}

impl Error {
    fn descr(&self) -> &str {
        match self {
            Error::_Serde => "", // not used
            Error::SerializeSequenceMustHaveLength => "serialized sequence must have length",
            Error::BufferOverflow => "serialized data buffer overflow",
            Error::BufferUnderflow => "serialized data buffer underflow",
            Error::PrematureEndOfInput => "premature end of input",
            Error::InvalidByteSequenceEscape => "invalid byte sequence escaping",
            Error::DeserializeAnyNotSupported => "deserialize to any type not supported",
            Error::DeserializeIdentifierNotSupported => "deserialize of identifiers not supported",
            Error::DeserializeIgnoredAny => "deserialize of ignored any not supported",
            Error::InvalidUtf8Encoding => "invalid UTF-8 encoding",
            Error::InvalidTagEncoding => "invalid encoding for enum tag",
            Error::InvalidVarintEncoding => "invalid varint encoding",
        }
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.descr())?;
        Ok(())
    }
}

#[cfg(feature="std")]
impl std::error::Error for Error {}

#[cfg(all(feature="serde", feature="std"))]
impl serde::ser::Error for Error {
    fn custom<T: std::fmt::Display>(_msg: T) -> Self {
        Self::_Serde
    }
}

#[cfg(all(feature="serde", feature="std"))]
impl serde::de::Error for Error {
    fn custom<T: std::fmt::Display>(_msg: T) -> Self {
        Self::_Serde
    }
}

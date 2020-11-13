
/// Serialization and deserialization errors
#[derive(Debug, Copy, Clone)]
pub enum Error {
    #[doc(hidden)]
    SerdeCustomError,  // not used, but need to satisfy serde Error traits
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
        #[cfg(feature="std")]
        match self {
            Error::SerdeCustomError => "serde custom error", // not used
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
        #[cfg(not(feature="std"))] ""
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.descr())?;
        Ok(())
    }
}

#[cfg(feature="std")]
impl std::error::Error for Error {}

#[cfg(feature="serde")]
const _: () =  {
    impl serde::ser::Error for Error {
        fn custom<T: core::fmt::Display>(_msg: T) -> Self {
            Self::SerdeCustomError
        }
    }
    impl serde::de::Error for Error {
        fn custom<T: core::fmt::Display>(_msg: T) -> Self {
            Self::SerdeCustomError
        }
    }
};
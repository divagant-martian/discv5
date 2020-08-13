use rlp::DecoderError;

#[derive(Debug)]
pub enum Discv5Error {
    InvalidEnr,
    UnknownPublicKey,
    KeyTypeNotSupported(&'static str),
    KeyDerivationFailed,
    InvalidRemotePublicKey,
    InvalidSecretKey,
    InvalidSignature,
    ServiceChannelClosed,
    ServiceNotStarted,
    ServiceAlreadyStarted,
    SessionNotEstablished,
    RLPError(DecoderError),
    EncryptionFail(String),
    DecryptionFailed(&'static str),
    Custom(&'static str),
    Error(String),
    Io(std::io::Error),
}

impl From<std::io::Error> for Discv5Error {
    fn from(err: std::io::Error) -> Discv5Error {
        Discv5Error::Io(err)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RequestError {
    Timeout,
    ServiceNotStarted,
    SelfRequest,
    ChannelFailed(String),
    InvalidEnr(String),
    InvalidRemoteEnr,
    InvalidRemotePacket,
    EncryptionFailed(String),
    InvalidMultiaddr(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum QueryError {
    ServiceNotStarted,
    ChannelFailed(String),
    InvalidEnr(String),
    EncryptionFailed(String),
    InvalidMultiaddr(String),
}

impl std::fmt::Display for Discv5Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::fmt::Display for RequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

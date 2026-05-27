use h3::error::{ConnectionError, StreamError};
use h3_quinn::quinn::{ConnectError, ConnectionError as ConnectionErrorQuinn};

#[derive(Debug)]
pub enum Error {
    H3Connect(ConnectError),
    H3Connection(ConnectionError),
    H3ConnectionQuinn(ConnectionErrorQuinn),
    H3Stream(StreamError),
}

impl From<StreamError> for Error {
    fn from(e: StreamError) -> Self {
        Self::H3Stream(e)
    }
}

impl From<ConnectError> for Error {
    fn from(e: ConnectError) -> Self {
        Self::H3Connect(e)
    }
}

impl From<ConnectionError> for Error {
    fn from(e: ConnectionError) -> Self {
        Self::H3Connection(e)
    }
}

impl From<ConnectionErrorQuinn> for Error {
    fn from(e: ConnectionErrorQuinn) -> Self {
        Self::H3ConnectionQuinn(e)
    }
}

//! The errors that various functions in the crate can throw.

use embedded_io::Error as EioError;

use crate::types;

/// An error arising from the underlying transport layer, the wifi layer, or
/// the TCP layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error<E: EioError> {
    Transport(E),
    NotIpv4,
    Delay,
    SetNetwork,
    SetPassphrase,
    SetKey,
    SetIpConfig,
    SetDnsConfig,
    SetHostname,
    Disconnect,
    ReqHostByName,
    StartScanNetworks,
    StartClientByIp,
    StopClient,
    CheckDataSent,
    PinMode,
    DigitalWrite,
    AnalogWrite,
    Tcp(TcpError),
}

/// An error from the TCP layer.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TcpError {
    ConnectionFailure(types::ConnectionState),
    BadConnectionStatus(num_enum::TryFromPrimitiveError<types::ConnectionState>),
    BadEncryptionType(num_enum::TryFromPrimitiveError<types::EncryptionType>),
    BadTcpState(num_enum::TryFromPrimitiveError<types::TcpState>),
    DataTooLong,
}

impl<E: EioError> From<TcpError> for Error<E> {
    fn from(value: TcpError) -> Self {
        Error::Tcp(value)
    }
}

impl<E: EioError> EioError for Error<E> {
    fn kind(&self) -> embedded_io::ErrorKind {
        embedded_io::ErrorKind::Other
    }
}

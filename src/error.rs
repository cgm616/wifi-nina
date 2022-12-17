use crate::types;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error<E> {
    Transport(E),
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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TcpError {
    ConnectionFailure(types::ConnectionState),
    BadConnectionStatus(num_enum::TryFromPrimitiveError<types::ConnectionState>),
    BadEncryptionType(num_enum::TryFromPrimitiveError<types::EncryptionType>),
    BadTcpState(num_enum::TryFromPrimitiveError<types::TcpState>),
    DataTooLong,
}

impl<E> From<TcpError> for Error<E> {
    fn from(value: TcpError) -> Self {
        Error::Tcp(value)
    }
}

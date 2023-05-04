//! Definitions of the commands that the `wifi-nina` firmware on
//! the coprocessor accepts over SPI.

/// A command that the `wifi-nina` firmware can take over SPI.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, num_enum::IntoPrimitive, num_enum::TryFromPrimitive,
)]
#[repr(u8)]
pub enum Command {
    SetNetCmd = 0x10,
    SetPassphraseCmd = 0x11,
    SetKeyCmd = 0x12,
    SetIpConfigCmd = 0x14,
    SetDnsConfigCmd = 0x15,
    SetHostnameCmd = 0x16,
    SetPowerModeCmd = 0x17,
    SetApNetCmd = 0x18,
    SetApPassphraseCmd = 0x19,
    SetDebugCmd = 0x1A,
    GetTemperatureCmd = 0x1B,

    GetConnStatusCmd = 0x20,
    GetIpaddrCmd = 0x21,
    GetMacaddrCmd = 0x22,
    GetCurrSsidCmd = 0x23,
    GetCurrBssidCmd = 0x24,
    GetCurrRssiCmd = 0x25,
    GetCurrEnctCmd = 0x26,
    ScanNetworks = 0x27,
    StartServerTcpCmd = 0x28,
    GetStateTcpCmd = 0x29,
    DataSentTcpCmd = 0x2A,
    AvailDataTcpCmd = 0x2B,
    GetDataTcpCmd = 0x2C,
    StartClientTcpCmd = 0x2D,
    StopClientTcpCmd = 0x2E,
    GetClientStateTcpCmd = 0x2F,
    DisconnectCmd = 0x30,
    GetIdxRssiCmd = 0x32,
    GetIdxEnctCmd = 0x33,
    ReqHostByNameCmd = 0x34,
    GetHostByNameCmd = 0x35,
    StartScanNetworks = 0x36,
    GetFwVersionCmd = 0x37,
    SendDataUdpCmd = 0x39,
    GetRemoteDataCmd = 0x3A,
    GetTimeCmd = 0x3B,
    GetIdxBssid = 0x3C,
    GetIdxChannelCmd = 0x3D,
    PingCmd = 0x3E,
    GetSocketCmd = 0x3F,

    // All command with DATA_FLAG 0x40 send a 16bit Len
    SendDataTcpCmd = 0x44,
    GetDatabufTcpCmd = 0x45,
    InsertDatabufCmd = 0x46,

    // regular format commands
    SetPinMode = 0x50,
    SetDigitalWrite = 0x51,
    SetAnalogWrite = 0x52,
}

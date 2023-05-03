#![doc = include_str!("../README.md")]
#![cfg_attr(not(test), no_std)]
#![feature(async_fn_in_trait)]
#![feature(impl_trait_projections)]
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

// Private modules
mod command;
mod encoding;
mod error;
mod handle;
mod param;
mod params;
mod util;

// Public modules
pub mod transport;
pub mod types;

// Public internal imports
pub use error::Error;

// Private internal imports
use handle::WifiNinaHandle;
use transport::Transport;
use types::{InternalSocket, ProtocolMode, SocketAddr};

// Core/std imports

// External crate imports
use arrayvec::ArrayVec;
use embedded_hal_async::delay::DelayUs;
use embedded_io::{
    asynch::{Read, Write},
    Io,
};
use embedded_nal_async::{Ipv4Addr, TcpConnect};
use lock_api::RawMutex;

pub struct WifiNina<MutexType: RawMutex, T: Transport> {
    handle: WifiNinaHandle<MutexType, T>,
    led_init: bool,
}

impl<MutexType: RawMutex, T: Transport> TcpConnect for WifiNina<MutexType, T> {
    type Error = error::Error<T::Error>;
    type Connection<'a> = Socket<'a, 4096, MutexType, T> where MutexType: 'a, T: 'a;

    async fn connect<'a>(&'a self, remote: SocketAddr) -> Result<Self::Connection<'a>, Self::Error>
    where
        Self: 'a,
    {
        // reject non-ipv4 addresses
        if !remote.is_ipv4() {
            return Err(error::Error::NotIpv4);
        }

        // ask the WifiNina for a socket
        let socket = self.handle.get_socket().await?;

        // start a new connection on that socket
        self.handle
            .start_client_by_addr(remote, socket, ProtocolMode::Tcp)
            .await?;

        // wrap it all up and return a new connection
        Ok(Socket {
            handle: &self.handle,
            socket,
            cursor: 0,
            buffer: [0; 4096],
        })
    }
}

impl<MutexType: RawMutex, T: Transport> WifiNina<MutexType, T> {
    pub fn new(transport: T) -> Self {
        let handle = handle::WifiNinaHandle::new(transport);
        Self {
            handle,
            led_init: false,
        }
    }

    pub async fn get_firmware_version(
        &mut self,
    ) -> Result<arrayvec::ArrayVec<u8, 16>, error::Error<T::Error>> {
        self.handle.get_firmware_version().await
    }

    pub async fn set_led(&mut self, r: u8, g: u8, b: u8) -> Result<(), error::Error<T::Error>> {
        if !self.led_init {
            self.handle.pin_mode(25, types::PinMode::Output).await?;
            self.handle.pin_mode(26, types::PinMode::Output).await?;
            self.handle.pin_mode(27, types::PinMode::Output).await?;
            self.led_init = true;
        }

        self.handle.analog_write(25, r).await?;
        self.handle.analog_write(26, g).await?;
        self.handle.analog_write(27, b).await?;

        Ok(())
    }

    pub async fn configure<DELAY: DelayUs>(
        &mut self,
        config: types::Config<'_>,
        delay: DELAY,
        connect_timeout: Option<(u32, u32)>,
    ) -> Result<(), error::Error<T::Error>> {
        match config {
            types::Config::Station(station_config) => match station_config.network {
                types::NetworkConfig::Open { ssid } => self.handle.set_network(ssid).await?,
                types::NetworkConfig::Password { ssid, password } => {
                    self.handle.set_passphrase(ssid, password).await?
                }
            },
            types::Config::AccessPoint(_) => unimplemented!(),
        }

        if let Some((timeout, interval)) = connect_timeout {
            self.await_connection_state(
                types::ConnectionState::Connected,
                delay,
                timeout,
                interval,
            )
            .await?;
        }

        Ok(())
    }

    pub async fn await_connection_state<DELAY: DelayUs>(
        &mut self,
        connection_state: types::ConnectionState,
        mut delay: DELAY,
        timeout: u32,
        interval: u32,
    ) -> Result<(), error::Error<T::Error>> {
        let mut total_time = 0;

        let mut actual_connection_state;
        loop {
            actual_connection_state = self.handle.get_connection_state().await?;
            if connection_state == actual_connection_state {
                return Ok(());
            }

            delay.delay_ms(interval).await;
            total_time += interval;

            if total_time > timeout {
                break;
            }
        }

        Err(error::TcpError::ConnectionFailure(actual_connection_state).into())
    }

    pub async fn scan_networks(
        &mut self,
    ) -> Result<ArrayVec<types::ScannedNetwork, 32>, error::Error<T::Error>> {
        self.handle.start_scan_networks().await?;

        let networks = self.handle.get_scanned_networks().await?;
        let mut network_info = ArrayVec::new();

        for (i, ssid) in networks.into_iter().enumerate() {
            let i = i as u8;
            let rssi = self.handle.get_scanned_network_rssi(i).await?;
            let encryption_type = self.handle.get_scanned_network_encryption_type(i).await?;
            let bssid = self.handle.get_scanned_network_bssid(i).await?;
            let channel = self.handle.get_scanned_network_channel(i).await?;

            network_info.push(types::ScannedNetwork {
                ssid,
                rssi,
                encryption_type,
                bssid,
                channel,
            });
        }

        Ok(network_info)
    }

    pub async fn ssid(&mut self) -> Result<arrayvec::ArrayVec<u8, 32>, error::Error<T::Error>> {
        self.handle.get_current_ssid().await
    }

    pub async fn bssid(&mut self) -> Result<arrayvec::ArrayVec<u8, 6>, error::Error<T::Error>> {
        self.handle.get_current_bssid().await
    }

    pub async fn rssi(&mut self) -> Result<i32, error::Error<T::Error>> {
        self.handle.get_current_rssi().await
    }

    pub async fn encryption_type(
        &mut self,
    ) -> Result<types::EncryptionType, error::Error<T::Error>> {
        self.handle.get_current_encryption_type().await
    }

    pub async fn resolve(&mut self, hostname: &str) -> Result<Ipv4Addr, error::Error<T::Error>> {
        self.handle.request_host_by_name(hostname).await?;
        self.handle.get_host_by_name().await
    }
}

pub struct Socket<'a, const BUFFER_CAPACITY: usize, MutexType: RawMutex, T: Transport> {
    handle: &'a WifiNinaHandle<MutexType, T>,
    socket: InternalSocket,
    cursor: usize,
    buffer: [u8; BUFFER_CAPACITY],
}

impl<'a, const BUFFER_CAPACITY: usize, MutexType: RawMutex, T: Transport>
    Socket<'a, BUFFER_CAPACITY, MutexType, T>
{
    pub async fn state(&self) -> Result<types::TcpState, error::Error<T::Error>> {
        self.handle.get_client_state(self.socket).await
    }
}

impl<'a, const BUFFER_CAPACITY: usize, MutexType: RawMutex, T: Transport> Io
    for Socket<'a, BUFFER_CAPACITY, MutexType, T>
{
    type Error = error::Error<T::Error>;
}

impl<'a, const BUFFER_CAPACITY: usize, MutexType: RawMutex, T: Transport> Read
    for Socket<'a, BUFFER_CAPACITY, MutexType, T>
{
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        // TODO: check what this function returns. get_data_buf() might just return
        // the length of the buffer---is that really how much data is recv'd?
        self.handle.get_data_buf(self.socket, buf).await
    }
}

impl<'a, const BUFFER_CAPACITY: usize, MutexType: RawMutex, T: Transport> Write
    for Socket<'a, BUFFER_CAPACITY, MutexType, T>
{
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        /// Helper function to fill `buffer` from `other` as much as possible, updating `cursor`
        fn fill_buffer(buffer: &mut [u8], cursor: &mut usize, other: &[u8]) -> usize {
            // Figure out how much to copy, either the length of other or the remaining space in the buffer
            let to_write = core::cmp::min(buffer.len() - *cursor, other.len());
            // Copy from other to the buffer, update the cursor, and return how much written
            buffer[*cursor..*cursor + to_write].copy_from_slice(&other[..to_write]);
            *cursor += to_write;
            to_write
        }

        assert!(self.cursor <= self.buffer.len());

        // Write as much as possible right off the bat
        let mut written = fill_buffer(&mut self.buffer, &mut self.cursor, buf);

        // Loop while the entire other buffer isn't written to the internal buffer
        while written < buf.len() {
            // If it isn't, that means the internal buffer is full; flush it
            self.flush().await?;
            // Then write as much as possible again
            written += fill_buffer(&mut self.buffer, &mut self.cursor, &buf[written..]);
        }

        Ok(written)
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        // How far we are through the buffer
        let mut flush_cursor = 0;

        // Loop until everything is sent
        while flush_cursor < self.cursor {
            // Calculate how much to send in this tranche
            let to_send = core::cmp::min(self.cursor - flush_cursor, u16::MAX as usize);
            // Send to the WifiNina, which will send it
            let sent = self
                .handle
                .send_data(
                    self.socket,
                    &self.buffer[flush_cursor..flush_cursor + to_send],
                )
                .await?;
            // Make sure it sent
            self.handle.check_data_sent(self.socket).await?;
            // Increase the cursor and move to the next chunk
            flush_cursor += sent;
        }

        // Reset cursor
        self.cursor = 0;

        Ok(())
    }
}

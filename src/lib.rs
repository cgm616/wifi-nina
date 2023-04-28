#![no_std]
#![feature(async_fn_in_trait)]
#![feature(impl_trait_projections)]
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

// Private modules
mod command;
mod encoding;
mod error;
mod handler;
mod param;
mod params;

use arrayvec::ArrayVec;
use embedded_hal_async::delay::DelayUs;
// Private internal imports
use transport::Transport;

// Public modules
pub mod nal;
pub mod transport;
pub mod types;

// Public internal imports
pub use error::Error;

// Core/std imports
use core::marker;

// External crate imports
use embedded_nal_async::Ipv4Addr;

// The length of the buffer to use for transmitting requests
const BUFFER_CAPACITY: usize = 4096;

#[derive(Debug)]
pub struct Wifi<T: Transport> {
    handler: handler::Handler<T>,
    led_init: bool,
}

#[derive(Debug)]
pub struct Client<T> {
    socket: types::Socket,
    buffer_offset: usize,
    buffer: arrayvec::ArrayVec<u8, BUFFER_CAPACITY>,
    phantom: marker::PhantomData<T>,
}

impl<T> Wifi<T>
where
    T: transport::Transport,
{
    pub fn new(transport: T) -> Self {
        let handler = handler::Handler::new(transport);
        let led_init = false;
        Self { handler, led_init }
    }

    pub async fn get_firmware_version(
        &mut self,
    ) -> Result<arrayvec::ArrayVec<u8, 16>, error::Error<T::Error>> {
        self.handler.get_firmware_version().await
    }

    pub async fn set_led(&mut self, r: u8, g: u8, b: u8) -> Result<(), error::Error<T::Error>> {
        if !self.led_init {
            self.handler.pin_mode(25, types::PinMode::Output).await?;
            self.handler.pin_mode(26, types::PinMode::Output).await?;
            self.handler.pin_mode(27, types::PinMode::Output).await?;
            self.led_init = true;
        }

        self.handler.analog_write(25, r).await?;
        self.handler.analog_write(26, g).await?;
        self.handler.analog_write(27, b).await?;

        Ok(())
    }

    pub async fn configure<DELAY: DelayUs>(
        &mut self,
        config: types::Config<'_>,
        delay: DELAY,
        connect_timeout: Option<u32>,
    ) -> Result<(), error::Error<T::Error>> {
        match config {
            types::Config::Station(station_config) => match station_config.network {
                types::NetworkConfig::Open { ssid } => self.handler.set_network(ssid).await?,
                types::NetworkConfig::Password { ssid, password } => {
                    self.handler.set_passphrase(ssid, password).await?
                }
            },
            types::Config::AccessPoint(_) => unimplemented!(),
        }

        if let Some(connect_timeout) = connect_timeout {
            self.await_connection_state(types::ConnectionState::Connected, delay, connect_timeout)
                .await?;
        }

        Ok(())
    }

    pub async fn await_connection_state<DELAY: DelayUs>(
        &mut self,
        connection_state: types::ConnectionState,
        mut delay: DELAY,
        timeout: u32,
    ) -> Result<(), error::Error<T::Error>> {
        const POLL_INTERVAL: u32 = 100;

        let mut total_time = 0;

        let mut actual_connection_state;
        loop {
            actual_connection_state = self.handler.get_connection_state().await?;
            if connection_state == actual_connection_state {
                return Ok(());
            }

            delay.delay_ms(POLL_INTERVAL).await;
            // TODO: don't assume the actual SPI transfer takes 0 time :)
            total_time += POLL_INTERVAL;

            if total_time > timeout {
                break;
            }
        }

        Err(error::TcpError::ConnectionFailure(actual_connection_state).into())
    }

    pub async fn scan_networks(
        &mut self,
    ) -> Result<ArrayVec<types::ScannedNetwork, 32>, error::Error<T::Error>> {
        self.handler.start_scan_networks().await?;

        let networks = self.handler.get_scanned_networks().await?;
        let mut network_info = ArrayVec::new();

        for (i, ssid) in networks.into_iter().enumerate() {
            let i = i as u8;
            let rssi = self.handler.get_scanned_network_rssi(i).await?;
            let encryption_type = self.handler.get_scanned_network_encryption_type(i).await?;
            let bssid = self.handler.get_scanned_network_bssid(i).await?;
            let channel = self.handler.get_scanned_network_channel(i).await?;

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
        self.handler.get_current_ssid().await
    }

    pub async fn bssid(&mut self) -> Result<arrayvec::ArrayVec<u8, 6>, error::Error<T::Error>> {
        self.handler.get_current_bssid().await
    }

    pub async fn rssi(&mut self) -> Result<i32, error::Error<T::Error>> {
        self.handler.get_current_rssi().await
    }

    pub async fn encryption_type(
        &mut self,
    ) -> Result<types::EncryptionType, error::Error<T::Error>> {
        self.handler.get_current_encryption_type().await
    }

    pub async fn resolve(&mut self, hostname: &str) -> Result<Ipv4Addr, error::Error<T::Error>> {
        self.handler.request_host_by_name(hostname).await?;
        self.handler.get_host_by_name().await
    }

    pub async fn new_client(&mut self) -> Result<Client<T>, error::Error<T::Error>> {
        let socket = self.handler.get_socket().await?;
        let buffer_offset = 0;
        let buffer = arrayvec::ArrayVec::new();
        let phantom = marker::PhantomData;
        Ok(Client {
            socket,
            buffer_offset,
            buffer,
            phantom,
        })
    }
}

impl<T> Client<T>
where
    T: transport::Transport,
{
    pub async fn connect_ipv4(
        &mut self,
        wifi: &mut Wifi<T>,
        ip: Ipv4Addr,
        port: u16,
        protocol_mode: types::ProtocolMode,
    ) -> Result<(), error::Error<T::Error>> {
        wifi.handler
            .start_client_by_ip(ip, port, self.socket, protocol_mode)
            .await
    }

    pub async fn send(
        &mut self,
        wifi: &mut Wifi<T>,
        data: &[u8],
    ) -> Result<usize, error::Error<T::Error>> {
        let len = data.len().min(u16::max_value() as usize);
        let sent = wifi.handler.send_data(self.socket, &data[..len]).await?;
        wifi.handler.check_data_sent(self.socket).await?;
        Ok(sent)
    }

    pub async fn send_all(
        &mut self,
        wifi: &mut Wifi<T>,
        mut data: &[u8],
    ) -> Result<(), error::Error<T::Error>> {
        while !data.is_empty() {
            let len = self.send(wifi, data).await?;
            data = &data[len..];
        }
        Ok(())
    }

    pub async fn state(
        &mut self,
        wifi: &mut Wifi<T>,
    ) -> Result<types::TcpState, error::Error<T::Error>> {
        wifi.handler.get_client_state(self.socket).await
    }

    pub async fn recv(
        &mut self,
        wifi: &mut Wifi<T>,
        data: &mut [u8],
    ) -> Result<usize, error::Error<T::Error>> {
        if self.buffer_offset >= self.buffer.len() {
            self.buffer.clear();
            self.buffer
                .try_extend_from_slice(&[0; BUFFER_CAPACITY])
                .unwrap();
            let recv_len = wifi
                .handler
                .get_data_buf(self.socket, self.buffer.as_mut())
                .await?;
            self.buffer.truncate(recv_len);
            self.buffer_offset = 0;
            defmt::debug!("fetched new buffer of len {}", self.buffer.len());
        }

        let len = data.len().min(self.buffer.len() - self.buffer_offset);
        data[..len].copy_from_slice(&self.buffer[self.buffer_offset..self.buffer_offset + len]);
        self.buffer_offset += len;
        Ok(len)
    }

    pub async fn recv_exact(
        &mut self,
        wifi: &mut Wifi<T>,
        mut data: &mut [u8],
    ) -> Result<(), error::Error<T::Error>> {
        while !data.is_empty() {
            let len = self.recv(wifi, data).await?;
            data = &mut data[len..];
        }
        Ok(())
    }
}

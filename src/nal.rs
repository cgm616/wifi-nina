use embedded_io::{
    asynch::{Read, Write},
    blocking::ReadExactError,
    Io,
};

use crate::{transport::Transport, Client, Error, Wifi};

/// A [`Client`] combined with the [`Wifi`] driver, allowing one to use the
/// [`embedded_io`] traits.
pub struct CombinedClient<'a, 'b, T> {
    wifi: &'a mut Wifi<T>,
    client: &'b mut Client<T>,
}

impl<'a, 'b, T> CombinedClient<'a, 'b, T> {
    pub fn new(wifi: &'a mut Wifi<T>, client: &'b mut Client<T>) -> Self {
        CombinedClient { wifi, client }
    }
}

impl<'a, 'b, T: Transport> Io for CombinedClient<'a, 'b, T> {
    type Error = Error<<T as Transport>::Error>;
}

impl<'a, 'b, T: Transport> Read for CombinedClient<'a, 'b, T> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.client.recv(self.wifi, buf)
    }

    async fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), ReadExactError<Self::Error>> {
        self.client
            .recv_exact(self.wifi, buf)
            .map_err(ReadExactError::Other)
    }
}

impl<'a, 'b, T: Transport> Write for CombinedClient<'a, 'b, T> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.client.send(self.wifi, buf)
    }
}

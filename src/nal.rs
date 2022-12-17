pub use embedded_nal::TcpClientStack;

use crate::{error::TcpError, transport::Transport, types::Socket, Client};

// impl<T: Transport> TcpClientStack for Client<T> {
//     type TcpSocket = Socket;
//     type Error = TcpError;

//     fn socket(&mut self) -> Result<Self::TcpSocket, Self::Error> {
//         self.
//     }
// }

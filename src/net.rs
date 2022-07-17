use crate::submit_op;
use std::io;
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::ptr::null_mut;

pub struct TcpListener {
    inner: std::net::TcpListener,
}

pub struct TcpStream {
    inner: std::net::TcpStream,
}

impl TcpListener {
    pub fn bind(addr: SocketAddr) -> io::Result<Self> {
        Ok(Self {
            inner: std::net::TcpListener::bind(addr)?,
        })
    }
    pub async fn accept(&self) -> io::Result<TcpStream> {
        let fd = io_uring::types::Fd(self.inner.as_raw_fd());
        let entry = io_uring::opcode::Accept::new(fd, null_mut(), null_mut()).build();

        let (entry, _) = unsafe { submit_op(entry, ()) }?.await;

        let fd = entry.result();

        if fd != -1 {
            let inner = unsafe { std::net::TcpStream::from_raw_fd(fd) };

            // needed for readiness io
            inner.set_nonblocking(true)?;

            Ok(TcpStream { inner })
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

impl TcpStream {
    // todo make nonblocking
    pub fn connect(addr: SocketAddr) -> io::Result<Self> {
        let inner = std::net::TcpStream::connect(addr)?;
        inner.set_nonblocking(true)?;

        Ok(Self { inner })
    }
    pub async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            match self.inner.read(buf) {
                Ok(len) => return Ok(len),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    let fd = io_uring::types::Fd(self.inner.as_raw_fd());
                    let entry = io_uring::opcode::PollAdd::new(fd, libc::POLLIN as _).build();

                    unsafe { submit_op(entry, ())?.await };
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
    }
    pub async fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        loop {
            match self.inner.write(buf) {
                Ok(len) => return Ok(len),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    let fd = io_uring::types::Fd(self.inner.as_raw_fd());
                    let entry = io_uring::opcode::PollAdd::new(fd, libc::POLLOUT as _).build();

                    unsafe { submit_op(entry, ())?.await };
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rt::Runtime;

    #[test]
    fn test_tcp_read() {
        let mut runtime = Runtime::new(256).unwrap();

        runtime.spawn(async {
            let listener = TcpListener::bind("127.0.0.1:8080".parse().unwrap()).unwrap();

            let mut stream = listener.accept().await.unwrap();

            let mut buf = [0; 64];

            let len = stream.read(&mut buf).await.unwrap();

            assert_eq!(b"hello", &buf[..len]);

            stream.write(b"world").await.unwrap();
        });

        runtime.spawn(async {
            let mut stream = TcpStream::connect("127.0.0.1:8080".parse().unwrap()).unwrap();

            stream.write(b"hello").await.unwrap();

            let mut buf = [0; 64];

            let len = stream.read(&mut buf).await.unwrap();

            assert_eq!(b"world", &buf[..len]);
        });

        runtime.run().unwrap();
    }
}

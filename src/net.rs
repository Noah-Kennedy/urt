use crate::submit_op;
use socket2::{Domain, Protocol, SockAddr, Type};
use std::io;
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd};
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
    pub async fn connect(addr: SocketAddr) -> io::Result<Self> {
        let socket = socket2::Socket::new(
            if addr.is_ipv4() {
                Domain::IPV4
            } else {
                Domain::IPV6
            },
            Type::STREAM,
            Some(Protocol::TCP),
        )?;

        let fd = io_uring::types::Fd(socket.as_raw_fd());

        let sock: Box<SockAddr> = Box::new(addr.into());

        let entry = io_uring::opcode::Connect::new(fd, sock.as_ptr(), sock.len()).build();

        let (entry, _) = unsafe { submit_op(entry, sock) }?.await;

        let ret = entry.result();

        if ret != -1 {
            let inner = unsafe { std::net::TcpStream::from_raw_fd(socket.into_raw_fd()) };

            // needed for readiness io
            inner.set_nonblocking(true)?;

            Ok(TcpStream { inner })
        } else {
            Err(io::Error::last_os_error())
        }
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

    // todo this is unsound
    pub async fn read_owned<T: AsMut<[u8]> + 'static + Unpin>(
        &mut self,
        mut buf: T,
    ) -> io::Result<(usize, T)> {
        if buf.as_mut().is_empty() {
            return Ok((0, buf));
        }

        let fd = io_uring::types::Fd(self.inner.as_raw_fd());

        let entry =
            io_uring::opcode::Read::new(fd, buf.as_mut().as_mut_ptr(), buf.as_mut().len() as _)
                .build();

        let (entry, buf) = unsafe { submit_op(entry, buf) }?.await;

        let len = entry.result();

        if len != -1 {
            Ok((len as usize, buf))
        } else {
            Err(io::Error::last_os_error())
        }
    }

    // todo this is unsound
    pub async fn write_owned<T: AsRef<[u8]> + 'static + Unpin>(
        &mut self,
        buf: T,
    ) -> io::Result<(usize, T)> {
        if buf.as_ref().is_empty() {
            return Ok((0, buf));
        }

        let fd = io_uring::types::Fd(self.inner.as_raw_fd());

        let entry =
            io_uring::opcode::Write::new(fd, buf.as_ref().as_ptr(), buf.as_ref().len() as _)
                .build();

        let (entry, buf) = unsafe { submit_op(entry, buf) }?.await;

        let len = entry.result();

        if len != -1 {
            Ok((len as usize, buf))
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rt::Runtime;

    #[test]
    fn test_tcp_readiness() {
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
            let mut stream = TcpStream::connect("127.0.0.1:8080".parse().unwrap())
                .await
                .unwrap();

            stream.write(b"hello").await.unwrap();

            let mut buf = [0; 64];

            let len = stream.read(&mut buf).await.unwrap();

            assert_eq!(b"world", &buf[..len]);
        });

        runtime.run().unwrap();
    }

    #[test]
    fn test_tcp_owned() {
        let mut runtime = Runtime::new(256).unwrap();

        runtime.spawn(async {
            let listener = TcpListener::bind("127.0.0.1:9000".parse().unwrap()).unwrap();

            let mut stream = listener.accept().await.unwrap();

            let buf = vec![0; 64];

            let (len, buf) = stream.read_owned(buf).await.unwrap();

            assert_eq!(b"hello", &buf[..len]);

            stream.write_owned(b"world").await.unwrap();
        });

        runtime.spawn(async {
            let mut stream = TcpStream::connect("127.0.0.1:9000".parse().unwrap())
                .await
                .unwrap();

            stream.write_owned(b"hello").await.unwrap();

            let buf = vec![0; 64];

            let (len, buf) = stream.read_owned(buf).await.unwrap();

            assert_eq!(b"world", &buf[..len]);
        });

        runtime.run().unwrap();
    }
}

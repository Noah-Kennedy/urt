use io_uring::squeue::Flags;
use tokio::io;
use urt::io::prepare_batch;
use urt::net::{TcpListener, TcpStream};
use urt::rt::Runtime;

const RESPONSE: &[u8] = b"HTTP/1.1 200 OK\r\nContent-length: 12\r\n\r\nHello world\n";

fn main() {
    let mut runtime = Runtime::new(256).unwrap();

    runtime.spawn(async {
        let listener = TcpListener::bind("[::1]:9000".parse().unwrap()).unwrap();

        loop {
            let stream = listener.accept().await.unwrap();

            urt::spawn(handle_connection(stream));
        }
    });

    runtime.run().unwrap();
}

async fn handle_connection(mut stream: TcpStream) -> io::Result<()> {
    let mut buf = vec![0; 4096];

    loop {
        unsafe {
            prepare_batch(2)?;

            let mut rx_op = stream.read_owned(buf);
            let tx_op = stream.write_owned(RESPONSE);

            rx_op.apply_flags(Flags::IO_HARDLINK);

            let rx_in_flight = rx_op.submit()?;
            let tx_in_flight = tx_op.submit()?;

            let (n, r_buf) = rx_in_flight.await?;

            let _ = tx_in_flight.await?;

            buf = r_buf;

            if n == 0 {
                break;
            }
        }
    }

    Ok(())
}

use futures::stream::FuturesUnordered;
use futures::StreamExt;
use io_uring::squeue::Flags;
use std::thread;
use tokio::io;
use urt::io::prepare_batch;
use urt::net::{TcpListener, TcpStream};
use urt::rt::Runtime;

const NUM_THREADS: usize = 1;
const RESPONSE: &[u8] = b"HTTP/1.1 200 OK\r\nContent-length: 12\r\n\r\nHello world\n";

fn main() {
    let mut threads = Vec::with_capacity(NUM_THREADS);

    for _ in 0..NUM_THREADS {
        threads.push(thread::spawn(run_server))
    }

    for handle in threads {
        handle.join().unwrap();
    }
}

fn run_server() {
    let mut runtime = Runtime::new(256).unwrap();

    runtime.spawn(async move {
        let listener = TcpListener::bind("[::1]:9000".parse().unwrap(), true).unwrap();

        let mut unordered = FuturesUnordered::new();

        for _ in 0..64 {
            unordered.push(listener.accept())
        }

        loop {
            let stream = unordered.next().await.unwrap().unwrap();

            unordered.push(listener.accept());

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

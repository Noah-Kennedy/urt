use tokio::io;
use urt::net::{TcpListener, TcpStream};
use urt::rt::Runtime;

const RESPONSE: &[u8] = b"HTTP/1.1 200 OK\r\nContent-length: 12\r\n\r\nHello world\n";

fn main() {
    let mut runtime = Runtime::new(256).unwrap();

    runtime.spawn(async {
        let listener = TcpListener::bind("[::1]:3000".parse().unwrap()).unwrap();

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
        let n = stream.read(&mut buf).await?;

        if n == 0 {
            break;
        }

        stream.write(RESPONSE).await?;
    }

    Ok(())
}

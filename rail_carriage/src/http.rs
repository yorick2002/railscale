use tokio::net::{TcpListener, TcpStream};
use std::net::SocketAddr;
use std::time::Instant;
use bytes::{Buf, Bytes, BytesMut};
use memchr::memmem::Finder;
use tokio::io::AsyncWriteExt;
use tokio_stream::StreamExt;
use crate::mapped_http_stream::MappedHttpStream;


async fn handle_stream(mut stream: TcpStream, _addr: SocketAddr) {
    let inst = Instant::now();
    let (read, mut write) = stream.split();
    let repl = vec![(Finder::new(b"Host"), Bytes::from_static(b"test"))];
    let mut mapped = MappedHttpStream::from(read, repl);
    while let Some(Ok(frame)) = mapped.next().await {
       // dbg!(&frame);
    }

    write.write_all(b"HTTP/1.1 200 OK\r\n\r\nSZIAAAAAOCSKIKE").await.unwrap();
    let done = inst.elapsed();
    println!("{}qs", done.as_nanos());
}

pub async fn run_http() {
    let tcp_listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();

    while let Ok((stream, addr)) = tcp_listener.accept().await {
        tokio::spawn(handle_stream(stream, addr));
    }
}

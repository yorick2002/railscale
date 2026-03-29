use std::collections::HashMap;
use std::io::Read;
use bytes::{Buf, Bytes, BytesMut};
use memchr::memmem::Finder;
use tokio_util::codec::Decoder;
use rayon::prelude::*;

fn find_crlf(buf: &[u8]) -> Option<usize> {
    let mut start = 0;
    loop {
        match memchr::memchr(b'\n', &buf[start..]) {
            Some(pos) => {
                let abs = start + pos;
                if abs > 0 && buf[abs - 1] == b'\r' {
                    return Some(abs - 1);
                }
                start = abs + 1;
            }
            None => return None,
        }
    }
}

pub struct HttpStreamingCodec {
    done: bool,
    pub matchers: Vec<(Finder<'static>, Bytes)>
}

impl HttpStreamingCodec {
    pub fn new(matchers: Vec<(Finder<'static>, Bytes)>) -> Self {
        Self { done: false, matchers }
    }
}

pub enum HttpFrame {
    HeadLine(Bytes),
    EndOfHeaders,
}

impl Decoder for HttpStreamingCodec {
    type Item = HttpFrame;
    type Error = tokio::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if self.done || src.is_empty() {
            return Ok(None);
        }


        match find_crlf(src) {
            Some(0) => {
                src.advance(2);
                self.done = true;
                Ok(Some(HttpFrame::EndOfHeaders))
            }
            Some(pos) => {
                let line = src.split_to(pos);
                src.advance(2);
                let line = self.matchers.par_iter().find_map_first(|(matcher, value)| {
                    let idx = matcher.find(&*line);
                    let sep = memchr::memchr(b':', &line);
                    if idx.is_some() && sep.is_some() {
                        let (name, _) = line.split_at(sep.unwrap());
                        println!("Header remapped {}:{:?}", String::from_utf8_lossy(name), value);
                        Some([name, b": ", value].concat().clone())
                    } else {
                        None
                    }
                }).unwrap_or(line.to_vec());
                Ok(Some(HttpFrame::HeadLine(Bytes::from_iter(line))))
            }
            None => Ok(None),
        }
    }
}


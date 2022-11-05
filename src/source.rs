use std::io::SeekFrom;

use tokio::io::{self, AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncSeek, AsyncSeekExt};

pub trait AsyncReadSeek: AsyncBufRead + AsyncRead + AsyncSeek + Unpin + Send + Sync {}

impl<T> AsyncReadSeek for T where T: AsyncBufRead + AsyncRead + AsyncSeek + Unpin + Send + Sync {}

#[derive(PartialEq, Clone)]
pub enum SourceType {
    Stdin,
    File(String),
}

pub struct Source {
    source_type: SourceType,
    source: Box<dyn AsyncReadSeek>,
    position: usize,
}

impl Source {
    pub fn new<T>(source_type: SourceType, source: T) -> Self
    // Somebody, somebody, please explain why did I have to use 'static here.
    where
        T: AsyncReadSeek + 'static,
    {
        Self {
            source_type: source_type,
            source: Box::new(source),
            position: 0,
        }
    }

    pub fn source_type(&self) -> SourceType {
        self.source_type.clone()
    }

    pub async fn read_line(&mut self) -> Option<String> {
        let res = self
            .source
            .seek(SeekFrom::Start(self.position as u64))
            .await;
        if res.is_err() {
            return None;
        }

        let mut buf = String::new();
        let size = self.source.read_line(&mut buf).await.unwrap_or(0);
        if size == 0 {
            return None;
        }
        self.position += size;
        Some(buf)
    }
}

// A wrapper for io::Stdin that additionaly provides AsyncSeek trait.
pub struct Stdin {
    inner: io::Stdin,
}

impl Stdin {
    pub fn new(stdin: io::Stdin) -> Self {
        Self { inner: stdin }
    }
}

impl AsyncSeek for Stdin {
    fn start_seek(self: std::pin::Pin<&mut Self>, _position: SeekFrom) -> std::io::Result<()> {
        Ok(())
    }

    fn poll_complete(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<u64>> {
        std::task::Poll::Ready(Ok(0))
    }
}

impl AsyncRead for Stdin {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let inner = unsafe { self.map_unchecked_mut(|s| &mut s.inner) };
        inner.poll_read(cx, buf)
    }
}

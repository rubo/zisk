use super::{FileStreamReader, NullStreamReader};

use anyhow::Result;

/// Core trait for stream reading operations
pub trait StreamRead: Send + 'static {
    /// Open/initialize the stream for reading
    fn open(&mut self) -> Result<()>;

    /// Read the next item from the stream
    /// Returns None when the stream is finished
    fn next(&mut self) -> Result<Option<Vec<u8>>>;

    /// Close the stream
    fn close(&mut self) -> Result<()>;

    /// Check if the stream is currently active
    fn is_active(&self) -> bool;
}

pub enum StreamSource {
    File(FileStreamReader),
    Null(NullStreamReader),
}

impl StreamSource {
    /// Create a null stdin
    pub fn null() -> Self {
        StreamSource::Null(NullStreamReader::new())
    }

    /// Create a file-based stdin
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        Ok(StreamSource::File(FileStreamReader::new(path)?))
    }
}

impl StreamRead for StreamSource {
    /// Open/initialize the stream for reading
    fn open(&mut self) -> Result<()> {
        match self {
            StreamSource::File(file_stream) => file_stream.open(),
            StreamSource::Null(null_stream) => null_stream.open(),
        }
    }

    /// Read the next item from the stream
    fn next(&mut self) -> Result<Option<Vec<u8>>> {
        match self {
            StreamSource::File(file_stream) => file_stream.next(),
            StreamSource::Null(null_stream) => null_stream.next(),
        }
    }

    /// Close the stream
    fn close(&mut self) -> Result<()> {
        match self {
            StreamSource::File(file_stream) => file_stream.close(),
            StreamSource::Null(null_stream) => null_stream.close(),
        }
    }

    /// Check if the stream is currently active
    fn is_active(&self) -> bool {
        match self {
            StreamSource::File(file_stream) => file_stream.is_active(),
            StreamSource::Null(null_stream) => null_stream.is_active(),
        }
    }
}

//! A file-based implementation of FileStreamReader.
//! This module provides functionality to read input data from a file.

use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};

use super::StreamRead;

use anyhow::Result;

/// A file-based implementation of ZiskStdin that reads from a file.
pub struct FileStreamReader {
    /// The path to the input file.
    path: PathBuf,

    /// Buffered reader for the file.
    reader: Option<BufReader<File>>,

    /// Track if the file has been read already.
    has_read: bool,
}

impl FileStreamReader {
    /// Create a new FileStreamReader from a file path.
    pub fn new<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        Ok(FileStreamReader { path: path.as_ref().to_path_buf(), reader: None, has_read: false })
    }
}

impl StreamRead for FileStreamReader {
    /// Open/initialize the stream for reading
    fn open(&mut self) -> Result<()> {
        let file = File::open(&self.path)?;
        self.reader = Some(BufReader::new(file));
        self.has_read = false;
        Ok(())
    }

    /// Reads the next item from the stream.
    ///
    /// This method does **not** stream incrementally. Instead, it repeatedly toggles
    /// between returning the full file contents and returning `None`, producing the
    /// following repeating sequence: `Some(Vec<u8>), None, Some(Vec<u8>), None, ...`
    fn next(&mut self) -> Result<Option<Vec<u8>>> {
        self.has_read = !self.has_read;

        if self.has_read {
            Ok(Some(fs::read(&self.path)?))
        } else {
            Ok(None)
        }
    }

    /// Close the stream
    fn close(&mut self) -> Result<()> {
        self.reader = None;
        Ok(())
    }

    /// Check if the stream is currently active
    fn is_active(&self) -> bool {
        self.reader.is_some()
    }
}

//! A file-based implementation of ZiskHintin.
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
}

impl FileStreamReader {
    /// Create a new FileHintin from a file path.
    pub fn new<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        Ok(FileStreamReader { path: path.as_ref().to_path_buf(), reader: None })
    }
}

impl StreamRead for FileStreamReader {
    /// Open/initialize the stream for reading
    fn open(&mut self) -> Result<()> {
        let file = File::open(&self.path)?;
        self.reader = Some(BufReader::new(file));
        Ok(())
    }

    /// Read the next item from the stream
    fn next(&mut self) -> Result<Vec<u8>> {
        Ok(fs::read(&self.path)?)
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

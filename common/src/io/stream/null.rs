use super::StreamRead;
use tracing::debug;

use anyhow::Result;

pub struct NullStreamReader;

impl StreamRead for NullStreamReader {
    /// Open/initialize the stream for reading
    fn open(&mut self) -> Result<()> {
        debug!("NullStreamReader opened - no data will be read");
        Ok(())
    }

    /// Read the next item from the stream
    fn next(&mut self) -> Result<Vec<u8>> {
        debug!("NullStreamReader next called - returning empty data");
        Ok(Vec::new())
    }

    /// Close the stream
    fn close(&mut self) -> Result<()> {
        debug!("NullStreamReader closed");
        Ok(())
    }

    /// Check if the stream is currently active
    fn is_active(&self) -> bool {
        false
    }
}

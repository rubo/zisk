mod file;
mod null;
mod stream_reader;
mod stream_writer;

use file::FileStreamReader;
use null::NullStreamReader;
pub use stream_reader::*;
pub use stream_writer::*;

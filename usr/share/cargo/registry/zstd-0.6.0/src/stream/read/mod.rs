//! Implement pull-based [`Read`] trait for both compressing and decompressing.
use std::io::{self, BufRead, BufReader, Read};

#[cfg(feature = "tokio")]
use tokio_io::AsyncRead;

use crate::dict::{DecoderDictionary, EncoderDictionary};
use crate::stream::{raw, zio};
use zstd_safe;

#[cfg(test)]
#[cfg(feature = "tokio")]
mod async_tests;

#[cfg(test)]
mod tests;

/// A decoder that decompress input data from another `Read`.
///
/// This allows to read a stream of compressed data
/// (good for files or heavy network stream).
pub struct Decoder<'a, R: BufRead> {
    reader: zio::Reader<R, raw::Decoder<'a>>,
}

/// An encoder that compress input data from another `Read`.
pub struct Encoder<'a, R: BufRead> {
    reader: zio::Reader<R, raw::Encoder<'a>>,
}

impl<R: Read> Decoder<'static, BufReader<R>> {
    /// Creates a new decoder.
    pub fn new(reader: R) -> io::Result<Self> {
        let buffer_size = zstd_safe::dstream_in_size();

        Self::with_buffer(BufReader::with_capacity(buffer_size, reader))
    }
}

impl<R: BufRead> Decoder<'static, R> {
    /// Creates a new decoder around a `BufRead`.
    pub fn with_buffer(reader: R) -> io::Result<Self> {
        Self::with_dictionary(reader, &[])
    }
    /// Creates a new decoder, using an existing dictionary.
    ///
    /// The dictionary must be the same as the one used during compression.
    pub fn with_dictionary(reader: R, dictionary: &[u8]) -> io::Result<Self> {
        let decoder = raw::Decoder::with_dictionary(dictionary)?;
        let reader = zio::Reader::new(reader, decoder);

        Ok(Decoder { reader })
    }
}
impl<'a, R: BufRead> Decoder<'a, R> {
    /// Sets this `Decoder` to stop after the first frame.
    ///
    /// By default, it keeps concatenating frames until EOF is reached.
    pub fn single_frame(mut self) -> Self {
        self.reader.set_single_frame();
        self
    }

    /// Creates a new decoder, using an existing `DecoderDictionary`.
    ///
    /// The dictionary must be the same as the one used during compression.
    pub fn with_prepared_dictionary<'b>(
        reader: R,
        dictionary: &DecoderDictionary<'b>,
    ) -> io::Result<Self>
    where
        'b: 'a,
    {
        let decoder = raw::Decoder::with_prepared_dictionary(dictionary)?;
        let reader = zio::Reader::new(reader, decoder);

        Ok(Decoder { reader })
    }

    /// Recommendation for the size of the output buffer.
    pub fn recommended_output_size() -> usize {
        zstd_safe::dstream_out_size()
    }

    /// Enables or disabled expecting the 4-byte magic header
    pub fn include_magicbytes(
        &mut self,
        include_magicbytes: bool,
    ) -> io::Result<()> {
        self.reader
            .operation_mut()
            .set_parameter(if include_magicbytes {
                zstd_safe::DParameter::Format(zstd_safe::FrameFormat::One)
            } else {
                zstd_safe::DParameter::Format(
                    zstd_safe::FrameFormat::Magicless,
                )
            })
    }

    /// Acquire a reference to the underlying reader.
    pub fn get_ref(&self) -> &R {
        self.reader.reader()
    }

    /// Acquire a mutable reference to the underlying reader.
    ///
    /// Note that mutation of the reader may result in surprising results if
    /// this decoder is continued to be used.
    pub fn get_mut(&mut self) -> &mut R {
        self.reader.reader_mut()
    }

    /// Return the inner `Read`.
    ///
    /// Calling `finish()` is not *required* after reading a stream -
    /// just use it if you need to get the `Read` back.
    pub fn finish(self) -> R {
        self.reader.into_inner()
    }
}

impl<R: BufRead> Read for Decoder<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.reader.read(buf)
    }
}

#[cfg(feature = "tokio")]
impl<R: AsyncRead + BufRead> AsyncRead for Decoder<'_, R> {
    unsafe fn prepare_uninitialized_buffer(&self, _buf: &mut [u8]) -> bool {
        false
    }
}

impl<R: Read> Encoder<'static, BufReader<R>> {
    /// Creates a new encoder.
    pub fn new(reader: R, level: i32) -> io::Result<Self> {
        let buffer_size = zstd_safe::dstream_in_size();

        Self::with_buffer(BufReader::with_capacity(buffer_size, reader), level)
    }
}

impl<R: BufRead> Encoder<'static, R> {
    /// Creates a new encoder around a `BufRead`.
    pub fn with_buffer(reader: R, level: i32) -> io::Result<Self> {
        Self::with_dictionary(reader, level, &[])
    }

    /// Creates a new encoder, using an existing dictionary.
    ///
    /// The dictionary must be the same as the one used during compression.
    pub fn with_dictionary(
        reader: R,
        level: i32,
        dictionary: &[u8],
    ) -> io::Result<Self> {
        let encoder = raw::Encoder::with_dictionary(level, dictionary)?;
        let reader = zio::Reader::new(reader, encoder);

        Ok(Encoder { reader })
    }
}

impl<'a, R: BufRead> Encoder<'a, R> {
    /// Creates a new encoder, using an existing `EncoderDictionary`.
    ///
    /// The dictionary must be the same as the one used during compression.
    pub fn with_prepared_dictionary<'b>(
        reader: R,
        dictionary: &EncoderDictionary<'b>,
    ) -> io::Result<Self>
    where
        'b: 'a,
    {
        let encoder = raw::Encoder::with_prepared_dictionary(dictionary)?;
        let reader = zio::Reader::new(reader, encoder);

        Ok(Encoder { reader })
    }

    /// Recommendation for the size of the output buffer.
    pub fn recommended_output_size() -> usize {
        zstd_safe::dstream_out_size()
    }

    /// Acquire a reference to the underlying reader.
    pub fn get_ref(&self) -> &R {
        self.reader.reader()
    }

    /// Acquire a mutable reference to the underlying reader.
    ///
    /// Note that mutation of the reader may result in surprising results if
    /// this encoder is continued to be used.
    pub fn get_mut(&mut self) -> &mut R {
        self.reader.reader_mut()
    }

    /// Return the inner `Read`.
    ///
    /// Calling `finish()` is not *required* after reading a stream -
    /// just use it if you need to get the `Read` back.
    pub fn finish(self) -> R {
        self.reader.into_inner()
    }

    crate::readwritecommon!(reader);
}

impl<R: BufRead> Read for Encoder<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.reader.read(buf)
    }
}

#[cfg(feature = "tokio")]
impl<R: AsyncRead + BufRead> AsyncRead for Encoder<'_, R> {
    unsafe fn prepare_uninitialized_buffer(&self, _buf: &mut [u8]) -> bool {
        false
    }
}

fn _assert_traits() {
    use std::io::Cursor;

    fn _assert_send<T: Send>(_: T) {}

    _assert_send(Decoder::new(Cursor::new(Vec::new())));
    _assert_send(Encoder::new(Cursor::new(Vec::new()), 1));
}

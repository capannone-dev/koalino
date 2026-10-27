//! Reads 16 kHz mono signed 16-bit PCM from a WAV file or a raw stream in 100 ms blocks.
//!
//! This is the CLI's audio source until a microphone adapter exists: `record start --input
//! meeting.wav` replays a file, and `--input -` reads raw little-endian PCM from stdin, e.g.
//! piped from `arecord -f S16_LE -r 16000 -c 1 -t raw`. Only the format the pipeline uses is
//! accepted; no resampling or channel mixing is done. Samples are never logged.

use std::io::{self, ErrorKind, Read};

/// Samples per block (100 ms at 16 kHz).
const BLOCK_SAMPLES: usize = 1_600;
/// The only accepted sample rate.
const REQUIRED_SAMPLE_RATE_HZ: u32 = 16_000;
/// Largest accepted `fmt ` chunk; real ones are 16 to 40 bytes. Bounds the header allocation.
const MAXIMUM_FORMAT_CHUNK_BYTES: u32 = 1_024;
/// Message used for every unsupported WAV format.
const UNSUPPORTED_FORMAT: &str = "WAV input must be 16 kHz mono 16-bit PCM";

/// A blocking source of 16 kHz mono PCM.
pub struct PcmInput {
    reader: Box<dyn Read + Send>,
    /// Bytes left in the WAV `data` chunk; `None` for raw streams, which end at EOF.
    remaining_data_bytes: Option<u64>,
}

impl PcmInput {
    /// Opens a WAV stream and positions it at the sample data.
    ///
    /// Inputs: A reader positioned at the RIFF header.
    /// Outputs: The input, or `InvalidData` when the stream is not a 16 kHz mono 16-bit PCM WAV.
    /// Side effects: Reads the header and skips non-audio chunks.
    pub fn wav(mut reader: Box<dyn Read + Send>) -> io::Result<Self> {
        let mut riff_header = [0_u8; 12];
        reader.read_exact(&mut riff_header)?;
        if &riff_header[0..4] != b"RIFF" || &riff_header[8..12] != b"WAVE" {
            return Err(invalid_data("input is not a RIFF/WAVE file"));
        }
        let mut format_checked = false;
        loop {
            let mut chunk_header = [0_u8; 8];
            reader.read_exact(&mut chunk_header)?;
            let chunk_length = u32::from_le_bytes([
                chunk_header[4],
                chunk_header[5],
                chunk_header[6],
                chunk_header[7],
            ]);
            match &chunk_header[0..4] {
                b"fmt " => {
                    if chunk_length > MAXIMUM_FORMAT_CHUNK_BYTES {
                        return Err(invalid_data(UNSUPPORTED_FORMAT));
                    }
                    let mut format = vec![0_u8; usize::try_from(chunk_length).unwrap_or(0)];
                    reader.read_exact(&mut format)?;
                    check_format(&format)?;
                    skip_bytes(&mut reader, u64::from(chunk_length % 2))?;
                    format_checked = true;
                }
                b"data" if format_checked => {
                    return Ok(Self {
                        reader,
                        remaining_data_bytes: Some(u64::from(chunk_length)),
                    });
                }
                b"data" => return Err(invalid_data("WAV data appears before its format")),
                _ => skip_bytes(
                    &mut reader,
                    u64::from(chunk_length) + u64::from(chunk_length % 2),
                )?,
            }
        }
    }

    /// Wraps a raw little-endian 16 kHz mono 16-bit PCM stream.
    ///
    /// Inputs: The stream (e.g. stdin).
    /// Outputs: The input; the format cannot be verified, so the caller vouches for it.
    /// Side effects: None.
    pub fn raw(reader: Box<dyn Read + Send>) -> Self {
        Self {
            reader,
            remaining_data_bytes: None,
        }
    }

    /// Reads the next block of up to 100 ms.
    ///
    /// Inputs: None.
    /// Outputs: The samples, `None` at the end of the input, or an I/O error. A trailing odd
    /// byte is dropped.
    /// Side effects: Blocks until a full block or the end of the input is read.
    pub fn read_block(&mut self) -> io::Result<Option<Vec<i16>>> {
        let wanted_bytes = match self.remaining_data_bytes {
            Some(remaining) => {
                usize::try_from(remaining.min(BLOCK_SAMPLES as u64 * 2)).unwrap_or(0)
            }
            None => BLOCK_SAMPLES * 2,
        };
        let mut block = vec![0_u8; wanted_bytes];
        let mut filled = 0;
        while filled < wanted_bytes {
            match self.reader.read(&mut block[filled..]) {
                Ok(0) => break,
                Ok(read_count) => filled += read_count,
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
        if let Some(remaining) = self.remaining_data_bytes.as_mut() {
            *remaining = remaining.saturating_sub(filled as u64);
        }
        let (sample_bytes, _odd_byte) = block[..filled].as_chunks::<2>();
        if sample_bytes.is_empty() {
            return Ok(None);
        }
        Ok(Some(
            sample_bytes
                .iter()
                .map(|little_endian| i16::from_le_bytes(*little_endian))
                .collect(),
        ))
    }
}

/// Accepts only uncompressed 16 kHz mono 16-bit PCM.
///
/// Inputs: The `fmt ` chunk body.
/// Outputs: `Ok(())`, or `InvalidData` naming the required format.
/// Side effects: None.
fn check_format(format: &[u8]) -> io::Result<()> {
    if format.len() < 16 {
        return Err(invalid_data(UNSUPPORTED_FORMAT));
    }
    let audio_format = u16::from_le_bytes([format[0], format[1]]);
    let channels = u16::from_le_bytes([format[2], format[3]]);
    let sample_rate_hz = u32::from_le_bytes([format[4], format[5], format[6], format[7]]);
    let bits_per_sample = u16::from_le_bytes([format[14], format[15]]);
    if audio_format != 1
        || channels != 1
        || sample_rate_hz != REQUIRED_SAMPLE_RATE_HZ
        || bits_per_sample != 16
    {
        return Err(invalid_data(UNSUPPORTED_FORMAT));
    }
    Ok(())
}

/// Discards `byte_count` bytes from `reader`.
///
/// Inputs: The reader and how many bytes to skip.
/// Outputs: `Ok(())`, or `UnexpectedEof` if the stream ends first.
/// Side effects: Advances the reader.
fn skip_bytes(reader: &mut Box<dyn Read + Send>, byte_count: u64) -> io::Result<()> {
    let skipped = io::copy(&mut reader.by_ref().take(byte_count), &mut io::sink())?;
    if skipped < byte_count {
        return Err(io::Error::new(
            ErrorKind::UnexpectedEof,
            "WAV file is truncated",
        ));
    }
    Ok(())
}

/// Builds an `InvalidData` error.
fn invalid_data(message: &str) -> io::Error {
    io::Error::new(ErrorKind::InvalidData, message.to_owned())
}

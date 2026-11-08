//! Compresses kept speech with Opus before it is sealed on disk or sent to a paired device (#88).
//!
//! Speech kept for the after-meeting pass used to be raw 16 kHz samples, about 115 MB per hour.
//! Opus at `BITRATE_BPS` keeps it at about a tenth of that; it is still sealed (and wiped from
//! memory) exactly as the raw samples were. Each chunk is encoded on its own, in 20 ms frames,
//! so a chunk decodes without the others. The encoder's lookahead delays its output; the decoder
//! drops that many samples first, so decoded speech lines up with the original sample for sample
//! and the chunk's times stay right.
//!
//! Layout of an encoded chunk: the sample count (u32 LE), the samples to drop at the start (u16
//! LE), then each frame as its length (u16 LE) and its Opus packet. libopus is the reference
//! implementation translated to Rust (`unsafe-libopus`); this module confines its unsafe calls to
//! two owned handles. Nothing here logs audio.

use unsafe_libopus::{
    OPUS_APPLICATION_AUDIO, OPUS_GET_LOOKAHEAD_REQUEST, OPUS_OK, OPUS_SET_BITRATE_REQUEST,
    OPUS_SET_COMPLEXITY_REQUEST, OPUS_SET_SIGNAL_REQUEST, OPUS_SIGNAL_VOICE, OpusDecoder,
    OpusEncoder, opus_decode, opus_decoder_create, opus_decoder_destroy, opus_encode,
    opus_encoder_create, opus_encoder_ctl_impl, opus_encoder_destroy,
};
use zeroize::Zeroizing;

/// The sample rate of kept speech, as the pipeline produces it.
const SAMPLE_RATE: i32 = 16_000;
/// Samples in one 20 ms frame.
const FRAME_SAMPLES: usize = 320;
/// Bits per second of the encoded speech.
pub const BITRATE_BPS: i32 = 24_000;
/// The largest Opus packet.
const MAX_PACKET_BYTES: usize = 1_275;
/// Bytes before the first frame: the sample count and the samples to drop.
const HEADER_BYTES: usize = 6;

/// Why speech could not be encoded or decoded.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum OpusSpeechError {
    /// The codec refused to start or to encode; the code is libopus's.
    #[error("the Opus codec failed ({0})")]
    Codec(i32),
    /// The encoded speech is cut short, damaged, or claims more than it holds.
    #[error("the encoded speech is malformed")]
    Malformed,
}

/// An Opus encoder, destroyed when dropped.
struct Encoder(*mut OpusEncoder);

impl Encoder {
    /// Creates a 16 kHz mono encoder for speech at `bitrate_bps`, at full complexity.
    fn new(bitrate_bps: i32) -> Result<Self, OpusSpeechError> {
        let mut error = 0;
        // SAFETY: valid rate, channel count and application; `error` outlives the call.
        let handle =
            unsafe { opus_encoder_create(SAMPLE_RATE, 1, OPUS_APPLICATION_AUDIO, &mut error) };
        if handle.is_null() || error != OPUS_OK {
            return Err(OpusSpeechError::Codec(error));
        }
        let encoder = Self(handle);
        for (request, value) in [
            (OPUS_SET_BITRATE_REQUEST, bitrate_bps),
            (OPUS_SET_SIGNAL_REQUEST, OPUS_SIGNAL_VOICE),
            (OPUS_SET_COMPLEXITY_REQUEST, 10),
        ] {
            // SAFETY: the handle is live; these requests take one `i32`.
            let status = unsafe {
                opus_encoder_ctl_impl(encoder.0, request, unsafe_libopus::varargs!(value))
            };
            if status != OPUS_OK {
                return Err(OpusSpeechError::Codec(status));
            }
        }
        Ok(encoder)
    }

    /// How many samples the encoder delays its output by.
    fn lookahead(&self) -> Result<usize, OpusSpeechError> {
        let mut lookahead = 0_i32;
        // SAFETY: the handle is live; this request writes one `i32` through the reference.
        let status = unsafe {
            opus_encoder_ctl_impl(
                self.0,
                OPUS_GET_LOOKAHEAD_REQUEST,
                unsafe_libopus::varargs!(&mut lookahead),
            )
        };
        if status != OPUS_OK {
            return Err(OpusSpeechError::Codec(status));
        }
        usize::try_from(lookahead).map_err(|_| OpusSpeechError::Codec(status))
    }

    /// Encodes one frame into `packet`, returning the packet's length.
    fn encode(
        &mut self,
        frame: &[i16; FRAME_SAMPLES],
        packet: &mut [u8; MAX_PACKET_BYTES],
    ) -> Result<usize, OpusSpeechError> {
        // SAFETY: the handle is live; `frame` holds `FRAME_SAMPLES` samples and `packet` holds
        // `MAX_PACKET_BYTES` bytes, the sizes passed.
        let length = unsafe {
            opus_encode(
                self.0,
                frame.as_ptr(),
                FRAME_SAMPLES as i32,
                packet.as_mut_ptr(),
                MAX_PACKET_BYTES as i32,
            )
        };
        usize::try_from(length).map_err(|_| OpusSpeechError::Codec(length))
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        // SAFETY: the handle came from `opus_encoder_create` and is destroyed once.
        unsafe { opus_encoder_destroy(self.0) }
    }
}

/// An Opus decoder, destroyed when dropped.
struct Decoder(*mut OpusDecoder);

impl Decoder {
    /// Creates a 16 kHz mono decoder.
    fn new() -> Result<Self, OpusSpeechError> {
        let mut error = 0;
        // SAFETY: valid rate and channel count; `error` outlives the call.
        let handle = unsafe { opus_decoder_create(SAMPLE_RATE, 1, &mut error) };
        if handle.is_null() || error != OPUS_OK {
            return Err(OpusSpeechError::Codec(error));
        }
        Ok(Self(handle))
    }

    /// Decodes one packet into `frame`; a packet that is not one 20 ms frame is malformed.
    fn decode(
        &mut self,
        packet: &[u8],
        frame: &mut [i16; FRAME_SAMPLES],
    ) -> Result<(), OpusSpeechError> {
        let length = i32::try_from(packet.len()).map_err(|_| OpusSpeechError::Malformed)?;
        // SAFETY: the handle is live; `packet` holds `length` bytes and `frame` room for
        // `FRAME_SAMPLES` samples, the sizes passed.
        let samples = unsafe {
            opus_decode(
                self.0,
                packet.as_ptr(),
                length,
                frame.as_mut_ptr(),
                FRAME_SAMPLES as i32,
                0,
            )
        };
        if samples == FRAME_SAMPLES as i32 {
            Ok(())
        } else {
            Err(OpusSpeechError::Malformed)
        }
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        // SAFETY: the handle came from `opus_decoder_create` and is destroyed once.
        unsafe { opus_decoder_destroy(self.0) }
    }
}

/// Encodes one chunk of 16 kHz mono speech.
///
/// Inputs: The samples (any length, including none).
/// Outputs: The encoded chunk (see the module header), wiped when dropped, or a codec error.
/// Side effects: None; the plaintext frame buffer is wiped.
pub fn encode_speech(samples: &[i16]) -> Result<Zeroizing<Vec<u8>>, OpusSpeechError> {
    encode_speech_at(samples, BITRATE_BPS)
}

/// Encodes one chunk as `encode_speech` does, at another bitrate (for measuring its effect).
///
/// Inputs: The samples and the bitrate in bits per second (500 to 512,000).
/// Outputs: The encoded chunk, or a codec error (also for a bitrate libopus refuses).
/// Side effects: None.
pub fn encode_speech_at(
    samples: &[i16],
    bitrate_bps: i32,
) -> Result<Zeroizing<Vec<u8>>, OpusSpeechError> {
    let mut encoder = Encoder::new(bitrate_bps)?;
    let lookahead = encoder.lookahead()?;
    let sample_count = u32::try_from(samples.len()).map_err(|_| OpusSpeechError::Malformed)?;
    let skip = u16::try_from(lookahead).map_err(|_| OpusSpeechError::Codec(lookahead as i32))?;
    // The lookahead is flushed with silence so the last samples come out too.
    let frame_count = (samples.len() + lookahead).div_ceil(FRAME_SAMPLES);
    let mut encoded = Zeroizing::new(Vec::with_capacity(
        HEADER_BYTES + frame_count * (2 + bitrate_bps.unsigned_abs() as usize / 8 / 50 + 8),
    ));
    encoded.extend_from_slice(&sample_count.to_le_bytes());
    encoded.extend_from_slice(&skip.to_le_bytes());
    let mut frame = Zeroizing::new([0_i16; FRAME_SAMPLES]);
    let mut packet = Zeroizing::new([0_u8; MAX_PACKET_BYTES]);
    for index in 0..frame_count {
        let start = index * FRAME_SAMPLES;
        frame.fill(0);
        if start < samples.len() {
            let end = (start + FRAME_SAMPLES).min(samples.len());
            frame[..end - start].copy_from_slice(&samples[start..end]);
        }
        let length = encoder.encode(&frame, &mut packet)?;
        let length_bytes =
            u16::try_from(length).map_err(|_| OpusSpeechError::Codec(length as i32))?;
        encoded.extend_from_slice(&length_bytes.to_le_bytes());
        encoded.extend_from_slice(&packet[..length]);
    }
    Ok(encoded)
}

/// Decodes one chunk made by `encode_speech`.
///
/// Inputs: The encoded chunk, which may come from a paired device.
/// Outputs: The samples, as many as were encoded and in time with them, wiped when dropped; or
/// `Malformed` for data cut short, damaged, or claiming more samples than its frames hold.
/// Side effects: None.
pub fn decode_speech(encoded: &[u8]) -> Result<Zeroizing<Vec<i16>>, OpusSpeechError> {
    let (header, mut rest) = encoded
        .split_first_chunk::<HEADER_BYTES>()
        .ok_or(OpusSpeechError::Malformed)?;
    let sample_count = u32::from_le_bytes([header[0], header[1], header[2], header[3]]) as usize;
    let skip = usize::from(u16::from_le_bytes([header[4], header[5]]));
    // Every frame takes at least its length and one byte, which bounds what the data can hold.
    let most_frames = rest.len() / 3;
    if sample_count + skip > most_frames * FRAME_SAMPLES {
        return Err(OpusSpeechError::Malformed);
    }
    let mut decoder = Decoder::new()?;
    let mut samples = Zeroizing::new(Vec::with_capacity(sample_count + skip + FRAME_SAMPLES));
    let mut frame = Zeroizing::new([0_i16; FRAME_SAMPLES]);
    while !rest.is_empty() {
        let (length_bytes, after_length) = rest
            .split_first_chunk::<2>()
            .ok_or(OpusSpeechError::Malformed)?;
        let length = usize::from(u16::from_le_bytes(*length_bytes));
        if length == 0 || length > MAX_PACKET_BYTES || after_length.len() < length {
            return Err(OpusSpeechError::Malformed);
        }
        let (packet, after_packet) = after_length.split_at(length);
        decoder.decode(packet, &mut frame)?;
        samples.extend_from_slice(&frame[..]);
        rest = after_packet;
    }
    if samples.len() < sample_count + skip {
        return Err(OpusSpeechError::Malformed);
    }
    samples.drain(..skip);
    samples.truncate(sample_count);
    Ok(samples)
}

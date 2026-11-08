//! Standard base64 with padding (RFC 4648 §4), enough to send images inline without a new
//! dependency.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encodes bytes as standard base64 with `=` padding and no line breaks.
///
/// Inputs: Any bytes. Outputs: The text. Side effects: None.
pub fn encode_base64(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        let group = (u32::from(first) << 16) | (u32::from(second) << 8) | u32::from(third);
        for (position, shift) in [18_u32, 12, 6, 0].into_iter().enumerate() {
            if position <= chunk.len() {
                text.push(char::from(ALPHABET[((group >> shift) & 0x3f) as usize]));
            } else {
                text.push('=');
            }
        }
    }
    text
}

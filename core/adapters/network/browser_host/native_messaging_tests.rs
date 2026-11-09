//! Specifies native messaging framing: what the browser writes to the host's stdin and reads
//! from its stdout.

use std::io::Cursor;

use super::native_messaging::{
    MAXIMUM_INCOMING_MESSAGE_BYTES, NativeMessagingError, read_message, write_message,
};

#[test]
fn a_written_message_reads_back() {
    let mut buffer = Vec::new();
    write_message(&mut buffer, &serde_json::json!({"type": "status"}))
        .unwrap_or_else(|error| panic!("message should be written: {error}"));
    let length = u32::from_ne_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);
    assert_eq!(length as usize, buffer.len() - 4);

    let read = read_message(&mut Cursor::new(buffer))
        .unwrap_or_else(|error| panic!("message should be read: {error}"));
    assert_eq!(read, Some(serde_json::json!({"type": "status"})));
}

#[test]
fn the_end_of_input_is_no_message() {
    let read = read_message(&mut Cursor::new(Vec::<u8>::new()))
        .unwrap_or_else(|error| panic!("end of input is not an error: {error}"));
    assert_eq!(read, None);
}

#[test]
fn an_oversized_length_is_refused_before_reading_the_body() {
    let length = u32::try_from(MAXIMUM_INCOMING_MESSAGE_BYTES + 1).unwrap_or(u32::MAX);
    let mut frame = length.to_ne_bytes().to_vec();
    frame.extend_from_slice(b"{}");
    assert!(matches!(
        read_message(&mut Cursor::new(frame)),
        Err(NativeMessagingError::TooLarge { .. })
    ));
}

#[test]
fn a_truncated_body_or_invalid_json_is_an_error() {
    let mut truncated = 10_u32.to_ne_bytes().to_vec();
    truncated.extend_from_slice(b"{}");
    assert!(read_message(&mut Cursor::new(truncated)).is_err());

    let mut not_json = 3_u32.to_ne_bytes().to_vec();
    not_json.extend_from_slice(b"abc");
    assert!(matches!(
        read_message(&mut Cursor::new(not_json)),
        Err(NativeMessagingError::InvalidJson)
    ));
}

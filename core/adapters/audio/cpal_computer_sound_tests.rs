//! Specifies how the default sink's monitor source is found and buffered on Linux (#108).

use super::cpal_computer_sound::{monitor_buffer_frames, monitor_source_for};

#[test]
fn the_monitor_of_the_default_sink_is_picked() {
    let devices = [
        "pulseaudio:alsa_output.pci-0000_00_1b.0.analog-stereo",
        "pulseaudio:alsa_input.pci-0000_00_1b.0.analog-stereo",
        "pulseaudio:alsa_output.usb-headset.analog-stereo.monitor",
        "pulseaudio:alsa_output.pci-0000_00_1b.0.analog-stereo.monitor",
    ];
    assert_eq!(
        monitor_source_for(
            "pulseaudio:alsa_output.pci-0000_00_1b.0.analog-stereo",
            &devices
        ),
        Some("pulseaudio:alsa_output.pci-0000_00_1b.0.analog-stereo.monitor")
    );
    assert_eq!(monitor_source_for("pulseaudio:missing", &devices), None);
}

#[test]
fn the_monitor_delivers_every_50_ms_within_the_supported_range() {
    // Without a size PulseAudio delivers a record stream in fragments of about 2 s, which put
    // the computer's sound 2 s late on the timeline and hid the echo from the detector.
    assert_eq!(monitor_buffer_frames(48_000, 1, 1_000_000), 2_400);
    assert_eq!(monitor_buffer_frames(44_100, 1, 1_000_000), 2_205);
    assert_eq!(monitor_buffer_frames(48_000, 4_096, 1_000_000), 4_096);
    assert_eq!(monitor_buffer_frames(48_000, 1, 1_000), 1_000);
}

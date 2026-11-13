//! Specifies how the default sink's monitor source is found on Linux (#108).

use super::cpal_computer_sound::monitor_source_for;

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

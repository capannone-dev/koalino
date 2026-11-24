// Asks the Rust core whether the recording's microphone is heard, so the Record screen can say
// when it seems muted, turned off, or blocked by Windows' privacy settings.

import 'dart:ffi';
import 'dart:isolate';

import 'package:note_taker_ui/infrastructure/core_library.dart';

/// What the open microphone gives the recording.
enum MicrophoneHearing {
  /// Sound arrives, or no microphone is open.
  heard,

  /// Only digital silence or no audio for a few seconds: muted, turned off, or blocked.
  silent,

  /// As `silent`, and Windows' privacy settings keep desktop apps from the microphone.
  blockedByWindows,
}

/// `FFI_MICROPHONE_SILENT` from `note_taker_microphone_silence`.
const int _microphoneSilentStatus = 1;

/// `FFI_MICROPHONE_BLOCKED_BY_WINDOWS` from `note_taker_microphone_silence`.
const int _microphoneBlockedByWindowsStatus = 2;

/// Maps `note_taker_microphone_silence`'s status to what the screen says.
///
/// Inputs: The status. Outputs: `silent` for 1, `blockedByWindows` for 2, otherwise `heard`
/// (an unavailable runtime says nothing rather than a false alarm). Side effects: None.
MicrophoneHearing microphoneHearingFromStatus(int status) => switch (status) {
      _microphoneSilentStatus => MicrophoneHearing.silent,
      _microphoneBlockedByWindowsStatus => MicrophoneHearing.blockedByWindows,
      _ => MicrophoneHearing.heard,
    };

/// Asks Rust whether the microphone is heard.
///
/// Outputs: The answer. Side effects: Runs on a background isolate, since the core's lock may be
/// held by a long stop and, on Windows, the first look in a silent spell reads the registry.
Future<MicrophoneHearing> readMicrophoneHearing() =>
    Isolate.run(() => microphoneHearingFromStatus(openCoreLibrary()
        .lookupFunction<Int32 Function(), int Function()>(
            'note_taker_microphone_silence')()));

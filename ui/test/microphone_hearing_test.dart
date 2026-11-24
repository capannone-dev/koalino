// Specifies how the core's microphone status becomes what the Record screen says.

import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/infrastructure/microphone_hearing.dart';
import 'package:note_taker_ui/presentation/recording_screen.dart';

void main() {
  test('each status maps to its hearing, and anything else to heard', () {
    expect(microphoneHearingFromStatus(0), MicrophoneHearing.heard);
    expect(microphoneHearingFromStatus(1), MicrophoneHearing.silent);
    expect(microphoneHearingFromStatus(2), MicrophoneHearing.blockedByWindows);
    expect(microphoneHearingFromStatus(3), MicrophoneHearing.heard,
        reason: 'an unavailable runtime raises no false alarm');
  });

  test('a heard microphone says nothing, and a call wins over a mute', () {
    expect(
        describeMicrophoneProblem(
            silencedByCall: false, hearing: MicrophoneHearing.heard),
        isNull);
    expect(
        describeMicrophoneProblem(
            silencedByCall: true, hearing: MicrophoneHearing.silent),
        contains('silencing the microphone'));
  });
}

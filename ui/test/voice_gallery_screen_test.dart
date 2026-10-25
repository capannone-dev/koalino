// Defines widget-first requirements for reviewing unassigned voice clips.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/voice_gallery_controller.dart';
import 'package:note_taker_ui/infrastructure/voice_gallery_bridge.dart';
import 'package:note_taker_ui/presentation/voice_gallery_screen.dart';

/// Captures native-boundary requests while keeping widget tests independent of Rust.
final class FakeVoiceGalleryBridge implements VoiceGalleryBridge {
  /// Creates a fake serving the supplied clips and identities.
  FakeVoiceGalleryBridge({required this.clips, required this.identities});

  /// Returns deterministic review clips to the controller.
  final List<VoiceClip> clips;

  /// Returns selectable speaker identities.
  final List<VoiceIdentity> identities;

  /// Records paths sent to native audio playback.
  final List<String> playedPaths = <String>[];

  /// Records user-confirmed weak-match requests.
  final List<(String, String)> confirmedMatches = <(String, String)>[];

  @override
  Future<void> assignNewIdentity({
    required String clipId,
    required String displayName,
  }) async {}

  @override
  Future<void> confirmWeakMatch({
    required String clipId,
    required String identityId,
  }) async {
    confirmedMatches.add((clipId, identityId));
  }

  @override
  Future<List<VoiceClip>> getUnassignedClips(String meetingId) async => clips;

  @override
  Future<List<VoiceIdentity>> getVoiceIdentities() async => identities;

  @override
  Future<void> playLocalFile(String localFilePath) async {
    playedPaths.add(localFilePath);
  }
}

/// Verifies Voice Gallery behavior through its injectable FFI boundary.
void main() {
  const VoiceClip weakClip = VoiceClip(
    clipId: 'clip-1',
    startTimeMs: 1250,
    endTimeMs: 2500,
    localFilePath: '/private/clips/clip-1.wav',
    weakMatch: WeakMatch(identityId: 'identity-1', confidenceScore: 0.67),
  );
  const VoiceIdentity suggestedIdentity = VoiceIdentity(
    identityId: 'identity-1',
    displayName: 'Ada Lovelace',
  );

  testWidgets('renders unassigned clips and plays their local audio files',
      (WidgetTester tester) async {
    final FakeVoiceGalleryBridge voiceGalleryBridge = FakeVoiceGalleryBridge(
      clips: <VoiceClip>[weakClip],
      identities: <VoiceIdentity>[suggestedIdentity],
    );
    final VoiceGalleryController voiceGalleryController =
        VoiceGalleryController(
      voiceGalleryBridge: voiceGalleryBridge,
      meetingId: 'meeting-1',
    );

    await tester.pumpWidget(MaterialApp(
      home: VoiceGalleryScreen(voiceGalleryController: voiceGalleryController),
    ));
    await tester.pump();

    expect(find.text('1.25s – 2.50s'), findsOneWidget);
    await tester.tap(find.text('Play'));
    await tester.pump();

    expect(
        voiceGalleryBridge.playedPaths, <String>['/private/clips/clip-1.wav']);
  });

  testWidgets('confirming a weak match delegates the selected identity to FFI',
      (WidgetTester tester) async {
    final FakeVoiceGalleryBridge voiceGalleryBridge = FakeVoiceGalleryBridge(
      clips: <VoiceClip>[weakClip],
      identities: <VoiceIdentity>[suggestedIdentity],
    );
    final VoiceGalleryController voiceGalleryController =
        VoiceGalleryController(
      voiceGalleryBridge: voiceGalleryBridge,
      meetingId: 'meeting-1',
    );

    await tester.pumpWidget(MaterialApp(
      home: VoiceGalleryScreen(voiceGalleryController: voiceGalleryController),
    ));
    await tester.pump();
    await tester.tap(find.text('Review weak match'));
    await tester.pumpAndSettle();

    expect(find.text('Is this Ada Lovelace?'), findsOneWidget);
    expect(find.text('67% confidence'), findsOneWidget);
    await tester.tap(find.text('Confirm'));
    await tester.pumpAndSettle();

    expect(voiceGalleryBridge.confirmedMatches, <(String, String)>[
      ('clip-1', 'identity-1'),
    ]);
  });
}

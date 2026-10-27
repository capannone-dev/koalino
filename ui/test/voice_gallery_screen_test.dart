// Defines widget-first requirements for reviewing and naming unknown-speaker clusters.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/voice_gallery_controller.dart';
import 'package:note_taker_ui/infrastructure/voice_gallery_bridge.dart';
import 'package:note_taker_ui/presentation/voice_gallery_screen.dart';

/// Captures native-boundary requests while keeping widget tests independent of Rust.
final class FakeVoiceGalleryBridge implements VoiceGalleryBridge {
  /// Creates a fake serving `clusters` until a cluster is assigned.
  FakeVoiceGalleryBridge({required List<UnknownSpeakerCluster> clusters})
      : _clusters = List<UnknownSpeakerCluster>.of(clusters);

  final List<UnknownSpeakerCluster> _clusters;

  /// Meeting IDs requested through `getUnknownClusters`.
  final List<String> requestedMeetings = <String>[];

  /// Paths sent to native audio playback.
  final List<String> playedPaths = <String>[];

  /// `(clusterId, identityName)` pairs sent to `assignCluster`.
  final List<(String, String)> assignedClusters = <(String, String)>[];

  /// When set, `assignCluster` throws this error instead of succeeding.
  Object? assignmentError;

  @override
  Future<List<UnknownSpeakerCluster>> getUnknownClusters(
      String meetingId) async {
    requestedMeetings.add(meetingId);
    return List<UnknownSpeakerCluster>.unmodifiable(_clusters);
  }

  @override
  Future<ClusterAssignmentResult> assignCluster({
    required String clusterId,
    required String identityName,
  }) async {
    assignedClusters.add((clusterId, identityName));
    final Object? error = assignmentError;
    if (error != null) {
      throw error;
    }
    _clusters.removeWhere(
        (UnknownSpeakerCluster cluster) => cluster.clusterId == clusterId);
    return ClusterAssignmentResult(
      clusterId: clusterId,
      identityId: 'identity-new',
      identityName: identityName,
      identityCreated: true,
      variantsStored: 2,
      segmentsUpdated: 1,
    );
  }

  @override
  Future<void> playLocalFile(String localFilePath) async {
    playedPaths.add(localFilePath);
  }
}

/// Verifies Voice Gallery behavior through its injectable FFI boundary.
void main() {
  const UnknownSpeakerCluster firstCluster = UnknownSpeakerCluster(
    clusterId: 'cluster-1',
    meetingId: 'meeting-1',
    totalDurationMs: 12500,
    chunkCount: 3,
    reviewClipPath: '/private/clips/clip-1.wav',
  );
  const UnknownSpeakerCluster secondCluster = UnknownSpeakerCluster(
    clusterId: 'cluster-2',
    meetingId: 'meeting-1',
    totalDurationMs: 700,
    chunkCount: 1,
  );

  /// Pumps the screen over `bridge` and waits for the initial load.
  Future<void> pumpGallery(
      WidgetTester tester, FakeVoiceGalleryBridge bridge) async {
    await tester.pumpWidget(MaterialApp(
      home: VoiceGalleryScreen(
        voiceGalleryController: VoiceGalleryController(
          voiceGalleryBridge: bridge,
          meetingId: 'meeting-1',
        ),
      ),
    ));
    await tester.pumpAndSettle();
  }

  testWidgets('renders one card per unknown-speaker cluster',
      (WidgetTester tester) async {
    final FakeVoiceGalleryBridge bridge = FakeVoiceGalleryBridge(
      clusters: <UnknownSpeakerCluster>[firstCluster, secondCluster],
    );

    await pumpGallery(tester, bridge);

    expect(bridge.requestedMeetings, <String>['meeting-1']);
    expect(find.text('Unknown speaker 1'), findsOneWidget);
    expect(find.text('12.5 s · 3 chunks'), findsOneWidget);
    expect(find.text('Unknown speaker 2'), findsOneWidget);
    expect(find.text('0.7 s · 1 chunk'), findsOneWidget);
    expect(find.widgetWithText(FilledButton, 'Assign'), findsNWidgets(2));
    expect(find.widgetWithText(TextButton, 'Play'), findsOneWidget);
  });

  testWidgets('plays a cluster review clip through its local file path',
      (WidgetTester tester) async {
    final FakeVoiceGalleryBridge bridge =
        FakeVoiceGalleryBridge(clusters: <UnknownSpeakerCluster>[firstCluster]);

    await pumpGallery(tester, bridge);
    await tester.tap(find.widgetWithText(TextButton, 'Play'));
    await tester.pump();

    expect(bridge.playedPaths, <String>['/private/clips/clip-1.wav']);
  });

  testWidgets('Assign sends the typed name for that cluster and refreshes',
      (WidgetTester tester) async {
    final FakeVoiceGalleryBridge bridge = FakeVoiceGalleryBridge(
      clusters: <UnknownSpeakerCluster>[firstCluster, secondCluster],
    );

    await pumpGallery(tester, bridge);
    await tester.enterText(
        find.byKey(const ValueKey<String>('identity-name-cluster-2')),
        '  Grace Hopper ');
    await tester.tap(find.byKey(const ValueKey<String>('assign-cluster-2')));
    await tester.pumpAndSettle();

    expect(bridge.assignedClusters, <(String, String)>[
      ('cluster-2', 'Grace Hopper'),
    ]);
    expect(bridge.requestedMeetings.length, 2);
    expect(find.text('Unknown speaker 2'), findsNothing);
    expect(find.text('Assigned to Grace Hopper'), findsOneWidget);
  });

  testWidgets('Assign with a blank name does not call the bridge',
      (WidgetTester tester) async {
    final FakeVoiceGalleryBridge bridge =
        FakeVoiceGalleryBridge(clusters: <UnknownSpeakerCluster>[firstCluster]);

    await pumpGallery(tester, bridge);
    await tester.enterText(
        find.byKey(const ValueKey<String>('identity-name-cluster-1')), '   ');
    await tester.tap(find.byKey(const ValueKey<String>('assign-cluster-1')));
    await tester.pumpAndSettle();

    expect(bridge.assignedClusters, isEmpty);
    expect(find.text('Enter a name before assigning'), findsOneWidget);
  });

  testWidgets('a rejected assignment keeps the cluster and shows an error',
      (WidgetTester tester) async {
    final FakeVoiceGalleryBridge bridge =
        FakeVoiceGalleryBridge(clusters: <UnknownSpeakerCluster>[firstCluster])
          ..assignmentError = StateError('cluster already resolved');

    await pumpGallery(tester, bridge);
    await tester.enterText(
        find.byKey(const ValueKey<String>('identity-name-cluster-1')), 'Ada');
    await tester.tap(find.byKey(const ValueKey<String>('assign-cluster-1')));
    await tester.pumpAndSettle();

    expect(bridge.assignedClusters, <(String, String)>[('cluster-1', 'Ada')]);
    expect(find.text('Unknown speaker 1'), findsOneWidget);
    expect(find.text('Unable to assign the speaker cluster'), findsOneWidget);
  });

  testWidgets('shows an empty state when no cluster needs review',
      (WidgetTester tester) async {
    await pumpGallery(
        tester, FakeVoiceGalleryBridge(clusters: <UnknownSpeakerCluster>[]));

    expect(find.text('No unknown speakers to review'), findsOneWidget);
  });

  group('FFI JSON decoding', () {
    test(
        'decodes the cluster array returned by note_taker_get_unknown_clusters',
        () {
      final List<UnknownSpeakerCluster> clusters = decodeUnknownClusters(
        '[{"cluster_id":"c1","meeting_id":"m1","total_duration_ms":2500,'
        '"chunk_count":2,"created_at":"2026-10-27T09:15:00+00:00",'
        '"review_clip_path":"/abs/clip.wav"},'
        '{"cluster_id":"c2","meeting_id":"m1","total_duration_ms":400,'
        '"chunk_count":1,"created_at":"2026-10-27T09:16:00+00:00"}]',
      );

      expect(clusters.length, 2);
      expect(clusters[0].clusterId, 'c1');
      expect(clusters[0].meetingId, 'm1');
      expect(clusters[0].totalDurationMs, 2500);
      expect(clusters[0].chunkCount, 2);
      expect(clusters[0].reviewClipPath, '/abs/clip.wav');
      expect(clusters[1].reviewClipPath, isNull);
    });

    test('turns a cluster listing error object into an exception', () {
      expect(
        () => decodeUnknownClusters(
            '{"ok":false,"error_code":"list_failed","error":"boom"}'),
        throwsA(isA<StateError>().having(
            (StateError error) => error.message, 'message', contains('boom'))),
      );
    });

    test('decodes a successful note_taker_assign_cluster response', () {
      final ClusterAssignmentResult result = decodeClusterAssignment(
        '{"ok":true,"cluster_id":"c1","identity_id":"i1",'
        '"identity_name":"Grace Hopper","identity_created":true,'
        '"variants_stored":2,"segments_updated":1}',
      );

      expect(result.clusterId, 'c1');
      expect(result.identityId, 'i1');
      expect(result.identityName, 'Grace Hopper');
      expect(result.identityCreated, isTrue);
      expect(result.variantsStored, 2);
      expect(result.segmentsUpdated, 1);
    });

    test('turns an assignment error object into an exception', () {
      expect(
        () => decodeClusterAssignment('{"ok":false,'
            '"error_code":"cluster_already_resolved","error":"resolved"}'),
        throwsA(isA<StateError>().having((StateError error) => error.message,
            'message', contains('cluster_already_resolved'))),
      );
    });
  });
}

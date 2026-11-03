// Defines widget-first requirements for reviewing and naming unknown-speaker clusters.

import 'dart:async';

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/voice_gallery_controller.dart';
import 'package:note_taker_ui/infrastructure/voice_gallery_bridge.dart';
import 'package:note_taker_ui/presentation/voice_gallery_screen.dart';

/// Captures native-boundary requests while keeping widget tests independent of Rust.
final class FakeVoiceGalleryBridge implements VoiceGalleryBridge {
  /// Creates a fake serving `clusters` until a cluster is assigned.
  FakeVoiceGalleryBridge({
    required List<UnknownSpeakerCluster> clusters,
    this.savedPeople = const <SavedPerson>[],
    this.variantsStored = 2,
  }) : _clusters = List<UnknownSpeakerCluster>.of(clusters);

  final List<UnknownSpeakerCluster> _clusters;

  /// How many voice samples an assignment reports saving.
  final int variantsStored;

  /// People returned with the library.
  List<SavedPerson> savedPeople;

  /// How many times the library was loaded.
  int libraryLoads = 0;

  /// When set, `getVoiceLibrary` throws this error.
  Object? loadError;

  /// Paths sent to native audio playback.
  final List<String> playedPaths = <String>[];

  /// How many times playback was stopped.
  int stopCount = 0;

  /// Tells listeners a clip played to its end.
  final StreamController<void> finishedController =
      StreamController<void>.broadcast();

  /// Clusters sent to `deleteUnknownVoice`.
  final List<String> deletedClusters = <String>[];

  /// People sent to `deleteSavedPerson`.
  final List<String> deletedPeople = <String>[];

  /// When set, the delete calls throw this error.
  Object? deleteError;

  /// `(clusterId, identityName)` pairs sent to `assignCluster`.
  final List<(String, String)> assignedClusters = <(String, String)>[];

  /// When set, `assignCluster` throws this error instead of succeeding.
  Object? assignmentError;

  /// `(comma-joined clusterIds, identityName)` pairs sent to `assignVoiceGroup`.
  final List<(String, String)> assignedGroups = <(String, String)>[];

  @override
  Future<VoiceLibrary> getVoiceLibrary() async {
    libraryLoads += 1;
    final Object? error = loadError;
    if (error != null) {
      throw error;
    }
    return VoiceLibrary(
      unknownClusters: List<UnknownSpeakerCluster>.unmodifiable(_clusters),
      savedPeople: savedPeople,
    );
  }

  @override
  Future<String> prepareReviewClip(String clusterId) async =>
      '/private/clips/$clusterId.wav';

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
      variantsStored: variantsStored,
      segmentsUpdated: 1,
    );
  }

  @override
  Future<VoiceGroupAssignmentResult> assignVoiceGroup({
    required List<String> clusterIds,
    required String identityName,
  }) async {
    assignedGroups.add((clusterIds.join(','), identityName));
    final Object? error = assignmentError;
    if (error != null) {
      throw error;
    }
    _clusters.removeWhere((UnknownSpeakerCluster cluster) =>
        clusterIds.contains(cluster.clusterId));
    return VoiceGroupAssignmentResult(
      identityId: 'identity-new',
      identityName: identityName,
      identityCreated: true,
      assignedClusterIds: clusterIds,
      skippedClusterCount: 0,
      variantsStored: variantsStored,
      segmentsUpdated: clusterIds.length,
    );
  }

  @override
  Future<void> playLocalFile(String localFilePath) async {
    playedPaths.add(localFilePath);
  }

  @override
  Future<void> stopPlayback() async {
    stopCount += 1;
  }

  @override
  Stream<void> get playbackFinished => finishedController.stream;

  @override
  Future<void> deleteUnknownVoice(String clusterId) async {
    if (deleteError case final Object error) {
      throw error;
    }
    deletedClusters.add(clusterId);
    _clusters.removeWhere(
        (UnknownSpeakerCluster cluster) => cluster.clusterId == clusterId);
  }

  @override
  Future<void> deleteSavedPerson(String identityId) async {
    if (deleteError case final Object error) {
      throw error;
    }
    deletedPeople.add(identityId);
    savedPeople = savedPeople
        .where((SavedPerson person) => person.identityId != identityId)
        .toList(growable: false);
  }
}

/// Verifies Voice Gallery behavior through its injectable FFI boundary.
void main() {
  const UnknownSpeakerCluster firstCluster = UnknownSpeakerCluster(
    clusterId: 'cluster-1',
    meetingId: 'meeting-1',
    totalDurationMs: 12500,
    chunkCount: 3,
    meetingTitle: 'Project Sync',
    hasReviewClip: true,
  );
  const UnknownSpeakerCluster secondCluster = UnknownSpeakerCluster(
    clusterId: 'cluster-2',
    meetingId: 'meeting-1',
    totalDurationMs: 700,
    chunkCount: 1,
  );

  const UnknownSpeakerCluster otherMeetingCluster = UnknownSpeakerCluster(
    clusterId: 'cluster-3',
    meetingId: 'meeting-2',
    totalDurationMs: 2000,
    chunkCount: 2,
  );
  const SavedPerson savedAda = SavedPerson(
    identityId: 'identity-1',
    displayName: 'Ada Lovelace',
    voiceVariantCount: 3,
  );

  /// Pumps the screen for `meetingId` (the whole gallery when null) and waits for the first load.
  Future<void> pumpGallery(WidgetTester tester, FakeVoiceGalleryBridge bridge,
      {String? meetingId = 'meeting-1'}) async {
    await tester.pumpWidget(MaterialApp(
      home: VoiceGalleryScreen(
        voiceGalleryController: VoiceGalleryController(
          voiceGalleryBridge: bridge,
          meetingId: meetingId,
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

    expect(bridge.playedPaths, <String>['/private/clips/cluster-1.wav']);
  });

  testWidgets('the voice playing is marked, and only that one',
      (WidgetTester tester) async {
    final FakeVoiceGalleryBridge bridge = FakeVoiceGalleryBridge(
      clusters: <UnknownSpeakerCluster>[
        firstCluster,
        const UnknownSpeakerCluster(
          clusterId: 'cluster-4',
          meetingId: 'meeting-1',
          totalDurationMs: 900,
          chunkCount: 1,
          hasReviewClip: true,
        ),
      ],
    );
    await pumpGallery(tester, bridge);
    expect(find.text('Playing…'), findsNothing);

    await tester.tap(find.byKey(const ValueKey<String>('play-cluster-1')));
    await tester.pumpAndSettle();

    expect(find.text('Playing…'), findsOneWidget);
    expect(
        find.descendant(
            of: find.byKey(const ValueKey<String>('card-cluster-1')),
            matching: find.text('Playing…')),
        findsOneWidget);
    expect(find.text('Stop'), findsOneWidget);
    expect(find.text('Play'), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey<String>('play-cluster-4')));
    await tester.pumpAndSettle();

    expect(
        find.descendant(
            of: find.byKey(const ValueKey<String>('card-cluster-4')),
            matching: find.text('Playing…')),
        findsOneWidget);
    expect(find.text('Playing…'), findsOneWidget);
    expect(bridge.stopCount, 1);
  });

  testWidgets('the mark goes when the clip ends or Stop is tapped',
      (WidgetTester tester) async {
    final FakeVoiceGalleryBridge bridge = FakeVoiceGalleryBridge(
      clusters: <UnknownSpeakerCluster>[firstCluster],
    );
    await pumpGallery(tester, bridge);

    await tester.tap(find.byKey(const ValueKey<String>('play-cluster-1')));
    await tester.pumpAndSettle();
    bridge.finishedController.add(null);
    await tester.pumpAndSettle();
    expect(find.text('Playing…'), findsNothing);
    expect(find.text('Play'), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey<String>('play-cluster-1')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey<String>('play-cluster-1')));
    await tester.pumpAndSettle();
    expect(find.text('Playing…'), findsNothing);
    expect(bridge.playedPaths, hasLength(2));
    expect(bridge.stopCount, 1);
  });

  testWidgets('deleting an unknown voice asks first, then removes it',
      (WidgetTester tester) async {
    final FakeVoiceGalleryBridge bridge = FakeVoiceGalleryBridge(
      clusters: <UnknownSpeakerCluster>[firstCluster, secondCluster],
    );
    await pumpGallery(tester, bridge);

    await tester.tap(find.byKey(const ValueKey<String>('delete-cluster-1')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    expect(bridge.deletedClusters, isEmpty);

    await tester.tap(find.byKey(const ValueKey<String>('delete-cluster-1')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Delete'));
    await tester.pumpAndSettle();

    expect(bridge.deletedClusters, <String>['cluster-1']);
    expect(find.byKey(const ValueKey<String>('card-cluster-1')), findsNothing);
    expect(
        find.byKey(const ValueKey<String>('card-cluster-2')), findsOneWidget);
    expect(find.text('Voice deleted'), findsOneWidget);
  });

  testWidgets('deleting the voice playing stops it',
      (WidgetTester tester) async {
    final FakeVoiceGalleryBridge bridge = FakeVoiceGalleryBridge(
      clusters: <UnknownSpeakerCluster>[firstCluster],
    );
    await pumpGallery(tester, bridge);
    await tester.tap(find.byKey(const ValueKey<String>('play-cluster-1')));
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const ValueKey<String>('delete-cluster-1')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Delete'));
    await tester.pumpAndSettle();

    expect(bridge.stopCount, 1);
    expect(find.text('Playing…'), findsNothing);
  });

  testWidgets('a saved person can be deleted, naming them in the question',
      (WidgetTester tester) async {
    final FakeVoiceGalleryBridge bridge = FakeVoiceGalleryBridge(
      clusters: const <UnknownSpeakerCluster>[],
      savedPeople: const <SavedPerson>[savedAda],
    );
    await pumpGallery(tester, bridge, meetingId: null);

    await tester
        .tap(find.byKey(const ValueKey<String>('delete-person-identity-1')));
    await tester.pumpAndSettle();
    expect(find.text('Delete Ada Lovelace?'), findsOneWidget);
    await tester.tap(find.text('Delete'));
    await tester.pumpAndSettle();

    expect(bridge.deletedPeople, <String>['identity-1']);
    expect(find.text('Ada Lovelace'), findsNothing);
  });

  testWidgets('a failed deletion keeps the voice and says so',
      (WidgetTester tester) async {
    final FakeVoiceGalleryBridge bridge = FakeVoiceGalleryBridge(
      clusters: <UnknownSpeakerCluster>[firstCluster],
    )..deleteError = StateError('rejected');
    await pumpGallery(tester, bridge);

    await tester.tap(find.byKey(const ValueKey<String>('delete-cluster-1')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Delete'));
    await tester.pumpAndSettle();

    expect(
        find.byKey(const ValueKey<String>('card-cluster-1')), findsOneWidget);
    expect(find.text('Unable to delete this voice'), findsOneWidget);
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
    expect(bridge.libraryLoads, 2);
    expect(find.text('Unknown speaker 2'), findsNothing);
    expect(find.text('Assigned to Grace Hopper'), findsOneWidget);
  });

  testWidgets('Assign says when no voice could be saved',
      (WidgetTester tester) async {
    final FakeVoiceGalleryBridge bridge = FakeVoiceGalleryBridge(
      clusters: <UnknownSpeakerCluster>[firstCluster],
      variantsStored: 0,
    );

    await pumpGallery(tester, bridge);
    await tester.enterText(
        find.byKey(const ValueKey<String>('identity-name-cluster-1')), 'Grace');
    await tester.tap(find.byKey(const ValueKey<String>('assign-cluster-1')));
    await tester.pumpAndSettle();

    expect(
        find.text('Assigned to Grace. There was not enough clear speech to '
            'recognize this voice in future recordings.'),
        findsOneWidget);
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

  testWidgets('from a meeting, shows only its voices and no saved people',
      (WidgetTester tester) async {
    await pumpGallery(
        tester,
        FakeVoiceGalleryBridge(
          clusters: <UnknownSpeakerCluster>[firstCluster, otherMeetingCluster],
          savedPeople: <SavedPerson>[savedAda],
        ));

    expect(find.text('12.5 s · 3 chunks'), findsOneWidget);
    expect(find.text('2.0 s · 2 chunks'), findsNothing);
    expect(find.text('Saved people'), findsNothing);
  });

  testWidgets('the whole gallery spans meetings and lists saved people',
      (WidgetTester tester) async {
    await pumpGallery(
        tester,
        FakeVoiceGalleryBridge(
          clusters: <UnknownSpeakerCluster>[firstCluster, otherMeetingCluster],
          savedPeople: <SavedPerson>[savedAda],
        ),
        meetingId: null);

    expect(find.text('Project Sync'), findsOneWidget);
    expect(find.text('Untitled meeting'), findsOneWidget);
    expect(find.text('Ada Lovelace'), findsOneWidget);
    expect(find.text('3 voice samples'), findsOneWidget);
  });

  testWidgets('a voice can be assigned to a saved person',
      (WidgetTester tester) async {
    final FakeVoiceGalleryBridge bridge = FakeVoiceGalleryBridge(
      clusters: <UnknownSpeakerCluster>[firstCluster],
      savedPeople: <SavedPerson>[savedAda],
    );

    await pumpGallery(tester, bridge, meetingId: null);
    await tester.tap(find.byTooltip('Choose a saved person'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Ada Lovelace').last);
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey<String>('assign-cluster-1')));
    await tester.pumpAndSettle();

    expect(bridge.assignedClusters, <(String, String)>[
      ('cluster-1', 'Ada Lovelace'),
    ]);
  });

  testWidgets('the whole gallery explains when both lists are empty',
      (WidgetTester tester) async {
    await pumpGallery(
        tester, FakeVoiceGalleryBridge(clusters: <UnknownSpeakerCluster>[]),
        meetingId: null);

    expect(find.text('No unknown speakers to review'), findsOneWidget);
    expect(find.text('No saved people yet. Name a voice to add someone.'),
        findsOneWidget);
  });

  testWidgets('a load failure offers Try again, which reloads',
      (WidgetTester tester) async {
    final FakeVoiceGalleryBridge bridge =
        FakeVoiceGalleryBridge(clusters: <UnknownSpeakerCluster>[firstCluster])
          ..loadError = StateError('no database');

    await pumpGallery(tester, bridge);
    expect(find.text('Unable to load Voice Gallery'), findsOneWidget);

    bridge.loadError = null;
    await tester.tap(find.text('Try again'));
    await tester.pumpAndSettle();

    expect(find.text('Unknown speaker 1'), findsOneWidget);
  });

  group('voices of one person (#82)', () {
    const UnknownSpeakerCluster longPart = UnknownSpeakerCluster(
      clusterId: 'part-1',
      meetingId: 'meeting-1',
      meetingTitle: 'Lunch',
      totalDurationMs: 105000,
      chunkCount: 1,
      hasReviewClip: true,
      voiceGroupId: 'part-1',
    );
    const UnknownSpeakerCluster shortPart = UnknownSpeakerCluster(
      clusterId: 'part-2',
      meetingId: 'meeting-1',
      meetingTitle: 'Lunch',
      totalDurationMs: 29000,
      chunkCount: 1,
      hasReviewClip: true,
      voiceGroupId: 'part-1',
    );
    const UnknownSpeakerCluster laterPart = UnknownSpeakerCluster(
      clusterId: 'part-3',
      meetingId: 'meeting-2',
      meetingTitle: 'Standup',
      totalDurationMs: 8000,
      chunkCount: 2,
      voiceGroupId: 'part-1',
    );
    const UnknownSpeakerCluster someoneElse = UnknownSpeakerCluster(
      clusterId: 'other-1',
      meetingId: 'meeting-1',
      meetingTitle: 'Lunch',
      totalDurationMs: 22000,
      chunkCount: 1,
      voiceGroupId: 'other-1',
    );
    final List<UnknownSpeakerCluster> lunch = <UnknownSpeakerCluster>[
      longPart,
      shortPart,
      laterPart,
      someoneElse,
    ];

    testWidgets('share one card that says how much it covers',
        (WidgetTester tester) async {
      final FakeVoiceGalleryBridge bridge =
          FakeVoiceGalleryBridge(clusters: lunch);

      await pumpGallery(tester, bridge, meetingId: null);

      expect(find.text('Unknown speaker 1'), findsOneWidget);
      expect(find.text('Unknown speaker 2'), findsOneWidget);
      expect(find.text('Unknown speaker 3'), findsNothing);
      expect(find.text('142.0 s · 3 parts in 2 meetings'), findsOneWidget);
      expect(find.text('Lunch, Standup'), findsOneWidget);
      expect(find.widgetWithText(FilledButton, 'Assign'), findsNWidgets(2));
    });

    testWidgets('are named at once', (WidgetTester tester) async {
      final FakeVoiceGalleryBridge bridge =
          FakeVoiceGalleryBridge(clusters: lunch);

      await pumpGallery(tester, bridge, meetingId: null);
      await tester.enterText(
          find.byKey(const ValueKey<String>('identity-name-part-1')),
          ' Marco ');
      await tester.tap(find.byKey(const ValueKey<String>('assign-part-1')));
      await tester.pumpAndSettle();

      expect(bridge.assignedGroups, <(String, String)>[
        ('part-1,part-2,part-3', 'Marco'),
      ]);
      expect(bridge.assignedClusters, isEmpty);
      expect(find.text('Assigned 3 voices to Marco'), findsOneWidget);
      expect(find.text('Unknown speaker 2'), findsNothing);
    });

    testWidgets('a part left out is not named and plays on its own',
        (WidgetTester tester) async {
      final FakeVoiceGalleryBridge bridge =
          FakeVoiceGalleryBridge(clusters: lunch);

      await pumpGallery(tester, bridge, meetingId: null);
      await tester.tap(find.text('3 parts'));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey<String>('play-part-2')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey<String>('include-part-2')));
      await tester.pumpAndSettle();
      await tester.enterText(
          find.byKey(const ValueKey<String>('identity-name-part-1')), 'Marco');
      await tester
          .ensureVisible(find.byKey(const ValueKey<String>('assign-part-1')));
      await tester.tap(find.byKey(const ValueKey<String>('assign-part-1')));
      await tester.pumpAndSettle();

      expect(bridge.playedPaths, <String>['/private/clips/part-2.wav']);
      expect(bridge.assignedGroups, <(String, String)>[
        ('part-1,part-3', 'Marco'),
      ]);
    });

    testWidgets('with every part left out, nothing is named',
        (WidgetTester tester) async {
      final FakeVoiceGalleryBridge bridge = FakeVoiceGalleryBridge(
          clusters: <UnknownSpeakerCluster>[longPart, shortPart]);

      await pumpGallery(tester, bridge, meetingId: null);
      await tester.tap(find.text('2 parts'));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey<String>('include-part-1')));
      await tester.tap(find.byKey(const ValueKey<String>('include-part-2')));
      await tester.enterText(
          find.byKey(const ValueKey<String>('identity-name-part-1')), 'Marco');
      await tester
          .ensureVisible(find.byKey(const ValueKey<String>('assign-part-1')));
      await tester.tap(find.byKey(const ValueKey<String>('assign-part-1')));
      await tester.pumpAndSettle();

      expect(bridge.assignedGroups, isEmpty);
      expect(find.text('Choose at least one voice to name'), findsOneWidget);
    });

    testWidgets('Play plays the longest part', (WidgetTester tester) async {
      final FakeVoiceGalleryBridge bridge = FakeVoiceGalleryBridge(
          clusters: <UnknownSpeakerCluster>[shortPart, longPart]);

      await pumpGallery(tester, bridge, meetingId: null);
      await tester.tap(find.byKey(const ValueKey<String>('play-group-part-1')));
      await tester.pumpAndSettle();

      expect(bridge.playedPaths, <String>['/private/clips/part-1.wav']);
    });

    testWidgets('deleting the card deletes every part',
        (WidgetTester tester) async {
      final FakeVoiceGalleryBridge bridge =
          FakeVoiceGalleryBridge(clusters: lunch);

      await pumpGallery(tester, bridge, meetingId: null);
      await tester.tap(find.byKey(const ValueKey<String>('delete-part-1')));
      await tester.pumpAndSettle();
      expect(find.text('Delete Unknown speaker 1?'), findsOneWidget);
      await tester.tap(find.text('Delete'));
      await tester.pumpAndSettle();

      expect(bridge.deletedClusters, <String>['part-1', 'part-2', 'part-3']);
      expect(find.text('Unknown speaker 2'), findsNothing);
    });
  });

  group('FFI JSON decoding', () {
    test('decodes the note_taker_get_voice_library response', () {
      final VoiceLibrary library = decodeVoiceLibrary(
        '{"ok":true,"unknown_clusters":[{"cluster_id":"c1","meeting_id":"m1",'
        '"meeting_title":"Sync","total_duration_ms":2500,"chunk_count":2,'
        '"created_at":"2026-10-27T09:15:00+00:00","has_review_clip":true},'
        '{"cluster_id":"c2","meeting_id":"m1","meeting_title":null,'
        '"total_duration_ms":400,"chunk_count":1,'
        '"created_at":"2026-10-27T09:16:00+00:00","has_review_clip":false}],'
        '"saved_people":[{"identity_id":"i1","display_name":"Ada",'
        '"voice_variant_count":3}]}',
      );

      expect(library.unknownClusters.length, 2);
      expect(library.unknownClusters[0].clusterId, 'c1');
      expect(library.unknownClusters[0].meetingTitle, 'Sync');
      expect(library.unknownClusters[0].totalDurationMs, 2500);
      expect(library.unknownClusters[0].hasReviewClip, isTrue);
      expect(library.unknownClusters[1].meetingTitle, isNull);
      expect(library.unknownClusters[0].voiceGroupId, 'c1',
          reason: 'without a group, a voice is its own');
      expect(library.savedPeople.single.displayName, 'Ada');
      expect(library.savedPeople.single.voiceVariantCount, 3);
    });

    test('turns a library error object into an exception', () {
      expect(
        () => decodeVoiceLibrary(
            '{"ok":false,"error_code":"list_failed","error":"boom"}'),
        throwsA(isA<StateError>().having(
            (StateError error) => error.message, 'message', contains('boom'))),
      );
    });

    test('decodes each voice\'s group', () {
      final VoiceLibrary library = decodeVoiceLibrary(
        '{"ok":true,"unknown_clusters":[{"cluster_id":"c2","meeting_id":"m1",'
        '"meeting_title":null,"total_duration_ms":400,"chunk_count":1,'
        '"created_at":"2026-10-27T09:16:00+00:00","has_review_clip":false,'
        '"voice_group_id":"c1"}],"saved_people":[]}',
      );

      expect(library.unknownClusters.single.voiceGroupId, 'c1');
    });

    test('decodes a note_taker_assign_voice_group response', () {
      final VoiceGroupAssignmentResult result = decodeVoiceGroupAssignment(
        '{"ok":true,"identity_id":"i1","identity_name":"Marco",'
        '"identity_created":false,"assigned_cluster_ids":["c1","c2"],'
        '"skipped_cluster_count":1,"variants_stored":2,"segments_updated":5}',
      );

      expect(result.identityName, 'Marco');
      expect(result.identityCreated, isFalse);
      expect(result.assignedClusterIds, <String>['c1', 'c2']);
      expect(result.skippedClusterCount, 1);
      expect(result.variantsStored, 2);
      expect(
          () => decodeVoiceGroupAssignment(
              '{"ok":false,"error_code":"invalid_argument","error":"x"}'),
          throwsStateError);
    });

    test('decodes the review clip path', () {
      expect(decodeReviewClipPath('{"ok":true,"review_clip_path":"/a/c.wav"}'),
          '/a/c.wav');
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

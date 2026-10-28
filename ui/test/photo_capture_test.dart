// Defines the multi-shot camera (Issue #54): each photo is attached to the meeting as soon as it
// is taken, its temporary file is deleted, and the screen stays open for the next shot.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/photo_capture_controller.dart';
import 'package:note_taker_ui/presentation/photo_capture_screen.dart';

import 'fake_photo_camera.dart';

/// A controller over fakes, with the deleted temporary paths collected in `deletedPaths`.
({
  PhotoCaptureController controller,
  FakePhotoCamera camera,
  FakeMeetingAttachmentBridge bridge,
  List<String> deletedPaths,
}) captureHarness() {
  final FakePhotoCamera camera = FakePhotoCamera();
  final FakeMeetingAttachmentBridge bridge = FakeMeetingAttachmentBridge();
  final List<String> deletedPaths = <String>[];
  final PhotoCaptureController controller = PhotoCaptureController(
    camera: camera,
    attachmentBridge: bridge,
    meetingId: 'meeting-live',
    deleteTemporaryPhoto: (String path) async => deletedPaths.add(path),
  );
  return (
    controller: controller,
    camera: camera,
    bridge: bridge,
    deletedPaths: deletedPaths,
  );
}

/// Verifies capture, attachment, cleanup and the screen's controls.
void main() {
  test('each photo is attached on its own and its temporary file deleted',
      () async {
    final harness = captureHarness();
    await harness.controller.open();
    expect(harness.controller.state, PhotoCameraState.ready);

    await harness.controller.takePhoto();
    await harness.controller.takePhoto();

    expect(
        harness.bridge.requests.map((request) => request.filePaths),
        <List<String>>[
          <String>['/tmp/camera/CAP1.jpg'],
          <String>['/tmp/camera/CAP2.jpg'],
        ]);
    expect(harness.bridge.requests.map((request) => request.meetingId),
        everyElement('meeting-live'));
    expect(harness.deletedPaths,
        <String>['/tmp/camera/CAP1.jpg', '/tmp/camera/CAP2.jpg']);
    expect(harness.controller.photos, hasLength(2));
    expect(harness.controller.errorMessage, isNull);
  });

  test('a camera that cannot open is reported as unavailable', () async {
    final harness = captureHarness();
    harness.camera.openError = StateError('no camera');

    await harness.controller.open();

    expect(harness.controller.state, PhotoCameraState.unavailable);
  });

  test('a photo that cannot be attached is reported and still deleted',
      () async {
    final harness = captureHarness();
    await harness.controller.open();
    harness.bridge.error = StateError('disk full');

    await harness.controller.takePhoto();

    expect(harness.controller.photos, isEmpty);
    expect(harness.controller.errorMessage, 'The photo could not be saved.');
    expect(harness.deletedPaths, <String>['/tmp/camera/CAP1.jpg']);
  });

  test('disposing the controller closes the camera', () async {
    final harness = captureHarness();
    await harness.controller.open();

    harness.controller.dispose();
    await Future<void>.delayed(Duration.zero);

    expect(harness.camera.isOpen, isFalse);
  });

  testWidgets('the screen takes several photos without closing',
      (WidgetTester tester) async {
    final harness = captureHarness();
    await tester.pumpWidget(MaterialApp(
      home: Builder(
        builder: (BuildContext context) => TextButton(
          onPressed: () => Navigator.of(context).push(MaterialPageRoute<void>(
              builder: (BuildContext context) =>
                  PhotoCaptureScreen(controller: harness.controller))),
          child: const Text('Open camera'),
        ),
      ),
    ));
    await tester.tap(find.text('Open camera'));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('camera-preview')), findsOneWidget);

    await tester.tap(find.byTooltip('Take photo'));
    await tester.pumpAndSettle();
    await tester.tap(find.byTooltip('Take photo'));
    await tester.pumpAndSettle();

    expect(find.text('2 photos added'), findsOneWidget);
    expect(harness.bridge.requests, hasLength(2));
    await tester.tap(find.text('Done'));
    await tester.pumpAndSettle();
    expect(find.text('Open camera'), findsOneWidget);
  });

  testWidgets('the screen explains when the camera is unavailable',
      (WidgetTester tester) async {
    final harness = captureHarness();
    harness.camera.openError = StateError('no camera');

    await tester.pumpWidget(
        MaterialApp(home: PhotoCaptureScreen(controller: harness.controller)));
    await tester.pumpAndSettle();

    expect(find.text('The camera is unavailable.'), findsOneWidget);
    expect(find.byTooltip('Take photo'), findsNothing);
  });
}

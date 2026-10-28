// Provides a camera, an attachment bridge and a file picker that need no device or Rust library,
// for the attachment and photo-capture tests (Issue #54).

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/infrastructure/meeting_attachment_bridge.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/infrastructure/photo_camera.dart';

/// Hands out numbered temporary photo paths and records its lifecycle.
final class FakePhotoCamera implements PhotoCamera {
  /// When set, `open` throws it.
  Object? openError;

  /// Whether `open` completed and `close` has not been called since.
  bool isOpen = false;

  /// How many photos were taken.
  int photoCount = 0;

  @override
  Future<void> open() async {
    if (openError case final Object error) {
      throw error;
    }
    isOpen = true;
  }

  @override
  Widget buildPreview() => const ColoredBox(
      key: Key('camera-preview'),
      color: Colors.black,
      child: SizedBox.expand());

  @override
  Future<String> takePicture() async {
    photoCount += 1;
    return '/tmp/camera/CAP$photoCount.jpg';
  }

  @override
  Future<void> close() async {
    isOpen = false;
  }
}

/// One `attachFiles` request received by the fake.
typedef AttachRequest = ({String meetingId, List<String> filePaths});

/// Attaches every path it is given, or fails while `error` is set.
final class FakeMeetingAttachmentBridge implements MeetingAttachmentBridge {
  /// Requests received, in order.
  final List<AttachRequest> requests = <AttachRequest>[];

  /// Paths reported as failed instead of attached.
  Set<String> failingPaths = <String>{};

  /// When set, `attachFiles` records the request and throws it.
  Object? error;

  @override
  Future<AttachFilesResult> attachFiles(
      String meetingId, List<String> filePaths) async {
    requests.add((meetingId: meetingId, filePaths: filePaths));
    if (error case final Object error) {
      throw error;
    }
    return AttachFilesResult(
      attached: <MeetingAttachment>[
        for (final String path in filePaths)
          if (!failingPaths.contains(path))
            MeetingAttachment(
              attachmentId: 'id-$path',
              fileName: path.split('/').last,
              filePath: '/data/attachments/$meetingId/${path.split('/').last}',
              mimeType:
                  path.endsWith('.pdf') ? 'application/pdf' : 'image/jpeg',
              meetingTimeMs: null,
            ),
      ],
      failedPaths: filePaths.where(failingPaths.contains).toList(),
    );
  }
}

/// Returns the paths the test set, as if the owner picked them.
final class FakeFilesPicker implements AttachmentFilesPicker {
  /// Paths returned by the next `pickFiles`; empty means cancelled.
  List<String> pickedPaths = <String>[];

  @override
  Future<List<String>> pickFiles() async => pickedPaths;
}

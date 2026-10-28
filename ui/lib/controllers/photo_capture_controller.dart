// Owns one multi-shot camera session for a meeting (Issue #54). Each photo is attached to the
// meeting as soon as it is taken, so a photo taken during a recording gets that moment's meeting
// time from Rust, and the camera's temporary file is deleted right after Rust copied it.

import 'dart:async';
import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:note_taker_ui/infrastructure/meeting_attachment_bridge.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/infrastructure/photo_camera.dart';

/// Where the camera session is.
enum PhotoCameraState {
  /// The camera is being opened.
  opening,

  /// The preview is live and photos can be taken.
  ready,

  /// No camera, or access was denied.
  unavailable,
}

/// Deletes the camera's temporary file at `path`.
typedef TemporaryPhotoDeleter = Future<void> Function(String path);

/// Takes photos and attaches them to one meeting.
final class PhotoCaptureController extends ChangeNotifier {
  /// Creates a controller for `meetingId`.
  ///
  /// Inputs: The camera, the attachment bridge, the meeting, and how to delete temporary photos
  /// (defaults to deleting the file).
  /// Outputs: A controller in the `opening` state. Side effects: None until `open`.
  PhotoCaptureController({
    required PhotoCamera camera,
    required MeetingAttachmentBridge attachmentBridge,
    required this.meetingId,
    TemporaryPhotoDeleter? deleteTemporaryPhoto,
  })  : _camera = camera,
        _attachmentBridge = attachmentBridge,
        _deleteTemporaryPhoto = deleteTemporaryPhoto ?? _deleteFile;

  /// The meeting the photos are attached to.
  final String meetingId;

  final PhotoCamera _camera;
  final MeetingAttachmentBridge _attachmentBridge;
  final TemporaryPhotoDeleter _deleteTemporaryPhoto;

  PhotoCameraState _state = PhotoCameraState.opening;
  final List<MeetingAttachment> _photos = <MeetingAttachment>[];
  bool _isTakingPhoto = false;
  String? _errorMessage;
  bool _isDisposed = false;

  /// Where the camera session is.
  PhotoCameraState get state => _state;

  /// Photos attached during this session, oldest first.
  List<MeetingAttachment> get photos =>
      List<MeetingAttachment>.unmodifiable(_photos);

  /// Whether a photo is being taken or saved; the shutter is disabled meanwhile.
  bool get isTakingPhoto => _isTakingPhoto;

  /// Why the last photo was not saved, or `null`.
  String? get errorMessage => _errorMessage;

  /// Builds the live preview of the open camera.
  Widget cameraPreview() => _camera.buildPreview();

  /// Opens the camera.
  ///
  /// Outputs: Completes in the `ready` or `unavailable` state.
  /// Side effects: Turns the camera on; notifies listeners.
  Future<void> open() async {
    try {
      await _camera.open();
      _state = PhotoCameraState.ready;
    } on Object {
      _state = PhotoCameraState.unavailable;
    }
    if (_isDisposed) {
      unawaited(_camera.close());
      return;
    }
    notifyListeners();
  }

  /// Takes one photo and attaches it to the meeting.
  ///
  /// Outputs: Completes once the photo is attached, or with `errorMessage` set.
  /// Side effects: Writes a temporary photo, asks Rust to copy it into the meeting, then deletes
  /// the temporary file whatever happened, so no photo is left outside the app's storage.
  /// Ignored while another photo is being taken or the camera is not ready.
  Future<void> takePhoto() async {
    if (_state != PhotoCameraState.ready || _isTakingPhoto) {
      return;
    }
    _isTakingPhoto = true;
    _errorMessage = null;
    notifyListeners();
    String? temporaryPath;
    try {
      temporaryPath = await _camera.takePicture();
      final AttachFilesResult result = await _attachmentBridge
          .attachFiles(meetingId, <String>[temporaryPath]);
      if (result.attached.isEmpty) {
        _errorMessage = 'The photo could not be saved.';
      }
      _photos.addAll(result.attached);
    } on Object {
      _errorMessage = 'The photo could not be saved.';
    } finally {
      if (temporaryPath case final String path) {
        await _deleteTemporaryPhoto(path).catchError((Object _) {});
      }
      _isTakingPhoto = false;
      if (!_isDisposed) {
        notifyListeners();
      }
    }
  }

  @override
  void dispose() {
    _isDisposed = true;
    unawaited(_camera.close());
    super.dispose();
  }
}

/// Deletes a file, ignoring one that is already gone.
Future<void> _deleteFile(String path) async {
  final File file = File(path);
  if (await file.exists()) {
    await file.delete();
  }
}

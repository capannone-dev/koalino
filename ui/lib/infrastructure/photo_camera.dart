// Wraps the device camera behind a small interface, so the multi-shot photo screen (Issue #54)
// can be tested without a camera. The `camera` plugin covers Android, iOS and Windows; Linux has
// no implementation, so the app offers file attachments only there.

import 'dart:io';

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:camera/camera.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Whether this platform has a camera implementation.
bool get isCameraSupported =>
    Platform.isAndroid || Platform.isIOS || Platform.isWindows;

/// A camera that shows a live preview and takes still photos.
abstract interface class PhotoCamera {
  /// Opens the camera.
  ///
  /// Inputs: None. Outputs: Completes once the preview can be shown; throws when no camera is
  /// available or access is denied.
  /// Side effects: Turns the camera on; Android asks for the camera permission the first time.
  Future<void> open();

  /// Builds the live preview of an open camera.
  Widget buildPreview();

  /// Takes one photo.
  ///
  /// Inputs: None. Outputs: The path of a temporary JPEG, which the caller deletes once it has
  /// been copied; throws when the camera is not open or the shot fails.
  /// Side effects: Writes the temporary file.
  Future<String> takePicture();

  /// Turns the camera off. Side effects: Releases the device; safe to call when closed.
  Future<void> close();
}

/// Uses the `camera` plugin, preferring the back camera, without audio.
final class PluginPhotoCamera implements PhotoCamera {
  /// Creates a closed camera.
  PluginPhotoCamera();

  CameraController? _cameraController;

  @override
  Future<void> open() async {
    final List<CameraDescription> cameras = await availableCameras();
    if (cameras.isEmpty) {
      throw StateError(l10n.thisDeviceHasNoCamera);
    }
    final CameraDescription camera = cameras.firstWhere(
      (CameraDescription camera) =>
          camera.lensDirection == CameraLensDirection.back,
      orElse: () => cameras.first,
    );
    final CameraController cameraController =
        CameraController(camera, ResolutionPreset.high, enableAudio: false);
    await cameraController.initialize();
    _cameraController = cameraController;
  }

  @override
  Widget buildPreview() => switch (_cameraController) {
        final CameraController cameraController =>
          Center(child: CameraPreview(cameraController)),
        null => const SizedBox.shrink(),
      };

  @override
  Future<String> takePicture() async {
    final CameraController? cameraController = _cameraController;
    if (cameraController == null) {
      throw StateError(l10n.theCameraIsNotOpen);
    }
    return (await cameraController.takePicture()).path;
  }

  @override
  Future<void> close() async {
    final CameraController? cameraController = _cameraController;
    _cameraController = null;
    await cameraController?.dispose();
  }
}

// Renders the multi-shot camera (Issue #54): a live preview, a shutter that can be pressed again
// and again, the photos taken so far, and Done to go back. Photos are attached as they are taken.

import 'dart:io';

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/photo_capture_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Size of each thumbnail in the strip of photos taken, in logical pixels.
const double _thumbnailSize = 56;

/// Takes photos for one meeting until the owner taps Done.
final class PhotoCaptureScreen extends StatefulWidget {
  /// Creates the screen over `controller`, which its owner disposes.
  ///
  /// Outputs: A full-screen camera page; popping it returns the number of photos taken.
  /// Side effects: Opens the camera through the controller when shown.
  const PhotoCaptureScreen({required this.controller, super.key});

  final PhotoCaptureController controller;

  @override
  State<PhotoCaptureScreen> createState() => _PhotoCaptureScreenState();
}

/// Opens the camera once the screen is shown.
final class _PhotoCaptureScreenState extends State<PhotoCaptureScreen> {
  @override
  void initState() {
    super.initState();
    widget.controller.open();
  }

  @override
  Widget build(BuildContext context) {
    final PhotoCaptureController controller = widget.controller;
    return ListenableBuilder(
      listenable: controller,
      builder: (BuildContext context, Widget? child) {
        final int photoCount = controller.photos.length;
        return Scaffold(
          appBar: AppBar(
            title: Text(l10n.takePhotos),
            actions: <Widget>[
              TextButton(
                onPressed: () => Navigator.of(context).pop(photoCount),
                child: Text(l10n.done),
              ),
            ],
          ),
          body: SafeArea(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Expanded(child: _preview(context, controller)),
                Padding(
                  padding: const EdgeInsets.all(16),
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: <Widget>[
                      if (controller.errorMessage case final String message)
                        Padding(
                          padding: const EdgeInsets.only(bottom: 8),
                          child: Text(message,
                              style: TextStyle(
                                  color: Theme.of(context).colorScheme.error)),
                        ),
                      Text(
                        switch (photoCount) {
                          0 => l10n.photosAreAddedToTheMeeting,
                          _ => l10n.photosAddedCount(photoCount),
                        },
                        textAlign: TextAlign.center,
                      ),
                      const SizedBox(height: 12),
                      Row(
                        children: <Widget>[
                          Expanded(child: _thumbnails(controller.photos)),
                          if (controller.state == PhotoCameraState.ready)
                            IconButton.filled(
                              tooltip: l10n.takePhoto,
                              iconSize: 40,
                              onPressed: controller.isTakingPhoto
                                  ? null
                                  : controller.takePhoto,
                              icon: controller.isTakingPhoto
                                  ? const SizedBox.square(
                                      dimension: 40,
                                      child: CircularProgressIndicator())
                                  : const Icon(Icons.camera_alt),
                            ),
                          const Expanded(child: SizedBox.shrink()),
                        ],
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        );
      },
    );
  }

  /// Builds the live preview, a progress indicator while opening, or the unavailable message.
  Widget _preview(BuildContext context, PhotoCaptureController controller) =>
      switch (controller.state) {
        PhotoCameraState.opening =>
          const Center(child: CircularProgressIndicator()),
        PhotoCameraState.unavailable => Center(
            child: Padding(
              padding: EdgeInsets.all(24),
              child: Text(
                l10n.theCameraIsUnavailable,
                textAlign: TextAlign.center,
              ),
            ),
          ),
        PhotoCameraState.ready => ColoredBox(
            color: Colors.black,
            child: controller.cameraPreview(),
          ),
      };

  /// Builds the newest photos first, as small thumbnails.
  Widget _thumbnails(List<MeetingAttachment> photos) => SizedBox(
        height: _thumbnailSize,
        child: ListView(
          scrollDirection: Axis.horizontal,
          children: <Widget>[
            for (final MeetingAttachment photo in photos.reversed)
              Padding(
                padding: const EdgeInsets.only(right: 8),
                child: ClipRRect(
                  borderRadius: BorderRadius.circular(8),
                  child: Image.file(
                    File(photo.filePath),
                    width: _thumbnailSize,
                    height: _thumbnailSize,
                    fit: BoxFit.cover,
                    cacheWidth: 168,
                    errorBuilder: (BuildContext context, Object error,
                            StackTrace? stackTrace) =>
                        const SizedBox.square(
                            dimension: _thumbnailSize,
                            child: Icon(Icons.image_outlined)),
                  ),
                ),
              ),
          ],
        ),
      );
}

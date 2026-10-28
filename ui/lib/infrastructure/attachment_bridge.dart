// Defines attachment-import file-picker and Rust FFI boundaries for the Flutter client.

import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';

import 'package:ffi/ffi.dart';
import 'package:file_picker/file_picker.dart';

/// Selects a local PDF or image without exposing platform picker APIs to widgets.
abstract interface class AttachmentFilePicker {
  /// Opens the platform picker for supported attachment types.
  ///
  /// Inputs: None.
  /// Outputs: An absolute selected path, or `null` when the user cancels.
  /// Side effects: Opens a platform-owned picker UI.
  Future<String?> pickAttachmentPath();
}

/// Invokes the standard cross-platform picker for PDFs and common image formats.
final class PlatformAttachmentFilePicker implements AttachmentFilePicker {
  /// Opens the native picker with the formats accepted by the attachment workflow.
  ///
  /// Inputs: None.
  /// Outputs: The first selected local file path, or `null` for cancellation/unavailable paths.
  /// Side effects: Opens a platform file-picker surface.
  @override
  Future<String?> pickAttachmentPath() async {
    final FilePickerResult? selection = await FilePicker.platform.pickFiles(
      type: FileType.custom,
      allowedExtensions: const <String>['pdf', 'jpg', 'jpeg', 'png'],
    );
    return selection?.files.singleOrNull?.path;
  }
}

/// Declares the raw JSON and status operations exposed by the Rust attachment boundary.
abstract interface class AttachmentBridge {
  /// Requests a JSON suggestion array for the selected local attachment path.
  ///
  /// Inputs: `filePath` is an absolute local path returned by `AttachmentFilePicker`.
  /// Outputs: The owned Rust JSON response; parsing belongs to the Flutter controller.
  /// Side effects: Rust reads attachment metadata and local meeting timeline metadata.
  Future<String> processAttachment(String filePath);

  /// Confirms one user-selected meeting link for a pending imported attachment.
  ///
  /// Inputs: Both opaque IDs must have been returned in the suggestion JSON.
  /// Outputs: Completes when Rust accepts the link, otherwise throws a state error.
  /// Side effects: Rust persists the attachment-to-meeting association.
  Future<void> confirmAttachmentLink({
    required String attachmentId,
    required String meetingId,
  });
}

/// Calls `note_taker_process_attachment` on a background isolate.
///
/// Inputs: The selected file path.
/// Outputs: The suggestion JSON, or `null` when Rust returned no response.
/// Side effects: Rust reads the file's metadata and scores every meeting in SQLite (and the first
/// native call unlocks the encryption key), so this must not run on the UI isolate. Frees both the
/// argument and the Rust-owned response inside the isolate.
Future<String?> _processAttachmentInIsolate(String filePath) => Isolate.run(() {
      final DynamicLibrary coreLibrary =
          RustAttachmentBridge._openCoreLibrary();
      final _DartProcessAttachment processAttachment = coreLibrary
          .lookupFunction<_NativeProcessAttachment, _DartProcessAttachment>(
              'note_taker_process_attachment');
      final _DartFreeString freeString =
          coreLibrary.lookupFunction<_NativeFreeString, _DartFreeString>(
              'note_taker_free_string');
      final Pointer<Utf8> nativeFilePath = filePath.toNativeUtf8();
      try {
        final Pointer<Utf8> responsePointer = processAttachment(nativeFilePath);
        if (responsePointer == nullptr) {
          return null;
        }
        try {
          return responsePointer.toDartString();
        } finally {
          freeString(responsePointer);
        }
      } finally {
        calloc.free(nativeFilePath);
      }
    });

/// Calls `note_taker_confirm_attachment_link` on a background isolate.
///
/// Inputs: The opaque attachment and meeting IDs. Outputs: The C status code.
/// Side effects: Rust persists the link in SQLite; buffers are freed inside the isolate.
Future<int> _confirmAttachmentLinkInIsolate(
        String attachmentId, String meetingId) =>
    Isolate.run(() {
      final _DartConfirmAttachmentLink confirmAttachmentLink =
          RustAttachmentBridge._openCoreLibrary().lookupFunction<
              _NativeConfirmAttachmentLink,
              _DartConfirmAttachmentLink>('note_taker_confirm_attachment_link');
      final Pointer<Utf8> nativeAttachmentId = attachmentId.toNativeUtf8();
      final Pointer<Utf8> nativeMeetingId = meetingId.toNativeUtf8();
      try {
        return confirmAttachmentLink(nativeAttachmentId, nativeMeetingId);
      } finally {
        calloc
          ..free(nativeAttachmentId)
          ..free(nativeMeetingId);
      }
    });

/// Converts Dart attachment commands to the narrow Rust C ABI.
final class RustAttachmentBridge implements AttachmentBridge {
  /// Resolves attachment symbols from `dynamicLibrary` or the platform core library.
  ///
  /// Inputs: `dynamicLibrary` supplies a test or embedded native library when present.
  /// Outputs: A ready adapter with no parsed Rust domain values.
  /// Side effects: Opens the configured Rust dynamic library when omitted.
  RustAttachmentBridge();

  /// Loads FFI-owned suggestion JSON and promptly releases the native allocation.
  ///
  /// Inputs: `filePath` identifies one user-selected local attachment.
  /// Outputs: An unparsed JSON array supplied by Rust.
  /// Side effects: Allocates short-lived UTF-8 buffers and calls the Rust core.
  @override
  Future<String> processAttachment(String filePath) async {
    final String? responseJson = await _processAttachmentInIsolate(filePath);
    if (responseJson == null) {
      throw StateError('Rust could not process the selected attachment.');
    }
    return responseJson;
  }

  /// Sends the selected opaque IDs to Rust without constructing Rust-domain values in Dart.
  ///
  /// Inputs: `attachmentId` and `meetingId` are non-empty opaque strings from Rust JSON.
  /// Outputs: Completes on a zero C status or throws when the core rejects the selection.
  /// Side effects: Allocates temporary UTF-8 buffers and may persist a local link in Rust.
  @override
  Future<void> confirmAttachmentLink({
    required String attachmentId,
    required String meetingId,
  }) async {
    if (await _confirmAttachmentLinkInIsolate(attachmentId, meetingId) != 0) {
      throw StateError('Rust rejected the attachment link.');
    }
  }

  /// Opens the shared Rust core library for supported Flutter platforms.
  ///
  /// Inputs: Reads the current runtime platform.
  /// Outputs: A loaded Rust core library.
  /// Side effects: Loads native executable code into the Flutter process.
  static DynamicLibrary _openCoreLibrary() {
    if (Platform.isAndroid || Platform.isLinux) {
      return DynamicLibrary.open('libnote_taker_core.so');
    }
    if (Platform.isWindows) {
      return DynamicLibrary.open('note_taker_core.dll');
    }
    throw UnsupportedError('The Rust attachment bridge is unavailable here.');
  }
}

typedef _NativeProcessAttachment = Pointer<Utf8> Function(Pointer<Utf8>);
typedef _DartProcessAttachment = Pointer<Utf8> Function(Pointer<Utf8>);
typedef _NativeConfirmAttachmentLink = Int32 Function(
    Pointer<Utf8>, Pointer<Utf8>);
typedef _DartConfirmAttachmentLink = int Function(Pointer<Utf8>, Pointer<Utf8>);
typedef _NativeFreeString = Void Function(Pointer<Utf8>);
typedef _DartFreeString = void Function(Pointer<Utf8>);

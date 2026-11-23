// Defines typed telemetry DTOs and the caller-freed Rust FFI bridge.

import 'dart:convert';
import 'dart:ffi';
import 'dart:isolate';

import 'package:ffi/ffi.dart';

import 'core_library.dart';

final class TelemetryDevice {
  const TelemetryDevice(
      {required this.deviceId,
      required this.supportsVad,
      required this.lastSeen});
  final String deviceId;
  final bool supportsVad;
  final int lastSeen;
  factory TelemetryDevice.fromJson(Map<String, Object?> json) =>
      TelemetryDevice(
          deviceId: json['device_id']! as String,
          supportsVad: json['supports_vad']! as bool,
          lastSeen: json['last_seen']! as int);
}

final class TelemetryMetric {
  const TelemetryMetric(
      {required this.taskType,
      required this.durationMs,
      required this.latencyMs,
      required this.powerMwh,
      required this.recordedAt});
  final String taskType;
  final int durationMs;
  final int latencyMs;
  final double powerMwh;
  final int recordedAt;
  factory TelemetryMetric.fromJson(Map<String, Object?> json) =>
      TelemetryMetric(
          taskType: json['task_type']! as String,
          durationMs: json['duration_ms']! as int,
          latencyMs: json['latency_ms']! as int,
          powerMwh: (json['power_mwh']! as num).toDouble(),
          recordedAt: json['recorded_at']! as int);
}

final class TelemetrySummary {
  const TelemetrySummary(
      {required this.transcriptionMs,
      required this.latencyMs,
      required this.tasks,
      required this.powerMwh});
  final int transcriptionMs;
  final int latencyMs;
  final int tasks;
  final double powerMwh;
  factory TelemetrySummary.fromJson(Map<String, Object?> json) =>
      TelemetrySummary(
          transcriptionMs: json['total_transcription_time_ms']! as int,
          latencyMs: json['average_latency_ms']! as int,
          tasks: json['total_tasks']! as int,
          powerMwh: (json['power_estimate_mwh']! as num).toDouble());
}

abstract interface class TelemetryBridge {
  Future<List<TelemetryDevice>> getDevices();
  Future<TelemetrySummary> getSummary();
  Future<List<TelemetryMetric>> getDeviceTelemetry(String deviceId);
}

/// Reads telemetry from the Rust core through the C ABI.
///
/// Every native call runs on a background isolate, because each one reads SQLite (and the first
/// one may unlock the encryption key), which would otherwise freeze the dashboard.
final class RustTelemetryBridge implements TelemetryBridge {
  /// Creates a bridge; the native library is opened lazily inside each worker isolate.
  RustTelemetryBridge();

  /// Lists the devices that have reported telemetry.
  ///
  /// Outputs: Devices as Rust lists them; throws `StateError` when Rust returns no response.
  /// Side effects: Reads SQLite on a background isolate.
  @override
  Future<List<TelemetryDevice>> getDevices() async =>
      (jsonDecode(await _readTelemetry('note_taker_get_devices', null)) as List)
          .map((Object? value) =>
              TelemetryDevice.fromJson((value! as Map).cast<String, Object?>()))
          .toList();

  /// Loads the aggregate telemetry summary.
  ///
  /// Outputs: The summary; throws `StateError` when Rust returns no response.
  /// Side effects: Reads SQLite on a background isolate.
  @override
  Future<TelemetrySummary> getSummary() async =>
      TelemetrySummary.fromJson((jsonDecode(await _readTelemetry(
              'note_taker_get_telemetry_summary', null)) as Map)
          .cast<String, Object?>());

  /// Loads the metric series recorded by `deviceId`.
  ///
  /// Inputs: A device ID from `getDevices`.
  /// Outputs: The device's metrics; throws `StateError` when Rust returns no response.
  /// Side effects: Reads SQLite on a background isolate.
  @override
  Future<List<TelemetryMetric>> getDeviceTelemetry(String deviceId) async =>
      ((jsonDecode(await _readTelemetry(
                  'note_taker_get_device_telemetry', deviceId))
              as Map)['metrics'] as List)
          .map((Object? value) =>
              TelemetryMetric.fromJson((value! as Map).cast<String, Object?>()))
          .toList();

  /// Opens the platform's Rust core library.
  static DynamicLibrary _open() => openCoreLibrary();
}

/// Calls the telemetry export `symbol` on a background isolate and returns its JSON.
///
/// Inputs: The C symbol, and `argument` for the one export that takes a device ID (`null` for the
/// argument-less ones).
/// Outputs: The response JSON; throws `StateError` when Rust returns a null pointer.
/// Side effects: Rust reads SQLite. The argument and the Rust-owned response are freed inside the
/// isolate, because native pointers and `DynamicLibrary` handles can't cross isolates.
Future<String> _readTelemetry(String symbol, String? argument) async {
  final String? responseJson = await Isolate.run(() {
    final DynamicLibrary coreLibrary = RustTelemetryBridge._open();
    final _DartFree freeString = coreLibrary
        .lookupFunction<_NativeFree, _DartFree>('note_taker_free_string');
    final Pointer<Utf8> responsePointer;
    if (argument == null) {
      responsePointer =
          coreLibrary.lookupFunction<_NativeGet, _DartGet>(symbol)();
    } else {
      final Pointer<Utf8> nativeArgument = argument.toNativeUtf8();
      try {
        responsePointer = coreLibrary
            .lookupFunction<_NativeSeries, _DartSeries>(symbol)(nativeArgument);
      } finally {
        calloc.free(nativeArgument);
      }
    }
    if (responsePointer == nullptr) {
      return null;
    }
    try {
      return responsePointer.toDartString();
    } finally {
      freeString(responsePointer);
    }
  });
  if (responseJson == null) {
    throw StateError('Rust telemetry is unavailable');
  }
  return responseJson;
}

typedef _NativeGet = Pointer<Utf8> Function();
typedef _DartGet = Pointer<Utf8> Function();
typedef _NativeSeries = Pointer<Utf8> Function(Pointer<Utf8>);
typedef _DartSeries = Pointer<Utf8> Function(Pointer<Utf8>);
typedef _NativeFree = Void Function(Pointer<Utf8>);
typedef _DartFree = void Function(Pointer<Utf8>);

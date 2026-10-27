// Defines typed telemetry DTOs and the caller-freed Rust FFI bridge.

import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'package:ffi/ffi.dart';

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

final class RustTelemetryBridge implements TelemetryBridge {
  RustTelemetryBridge({DynamicLibrary? library})
      : _library = library ?? _open() {
    _devices =
        _library.lookupFunction<_NativeGet, _DartGet>('note_taker_get_devices');
    _summary = _library.lookupFunction<_NativeGet, _DartGet>(
        'note_taker_get_telemetry_summary');
    _series = _library.lookupFunction<_NativeSeries, _DartSeries>(
        'note_taker_get_device_telemetry');
    _free = _library
        .lookupFunction<_NativeFree, _DartFree>('note_taker_free_string');
  }
  final DynamicLibrary _library;
  late final _DartGet _devices;
  late final _DartGet _summary;
  late final _DartSeries _series;
  late final _DartFree _free;
  String _read(Pointer<Utf8> value) {
    if (value == nullptr) throw StateError('Rust telemetry is unavailable');
    try {
      return value.toDartString();
    } finally {
      _free(value);
    }
  }

  @override
  Future<List<TelemetryDevice>> getDevices() async =>
      (jsonDecode(_read(_devices())) as List)
          .map((Object? value) =>
              TelemetryDevice.fromJson((value! as Map).cast<String, Object?>()))
          .toList();
  @override
  Future<TelemetrySummary> getSummary() async => TelemetrySummary.fromJson(
      (jsonDecode(_read(_summary())) as Map).cast<String, Object?>());
  @override
  Future<List<TelemetryMetric>> getDeviceTelemetry(String deviceId) async {
    final Pointer<Utf8> input = deviceId.toNativeUtf8();
    try {
      return (jsonDecode(_read(_series(input))) as Map)['metrics']
          .cast<Object?>()
          .map((Object? value) =>
              TelemetryMetric.fromJson((value! as Map).cast<String, Object?>()))
          .toList();
    } finally {
      calloc.free(input);
    }
  }

  static DynamicLibrary _open() => DynamicLibrary.open(
      Platform.isWindows ? 'note_taker_core.dll' : 'libnote_taker_core.so');
}

typedef _NativeGet = Pointer<Utf8> Function();
typedef _DartGet = Pointer<Utf8> Function();
typedef _NativeSeries = Pointer<Utf8> Function(Pointer<Utf8>);
typedef _DartSeries = Pointer<Utf8> Function(Pointer<Utf8>);
typedef _NativeFree = Void Function(Pointer<Utf8>);
typedef _DartFree = void Function(Pointer<Utf8>);

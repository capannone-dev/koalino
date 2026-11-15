// Owns periodic telemetry polling while keeping Flutter widgets free of FFI calls.

import 'dart:async';
import 'package:flutter/foundation.dart';
import 'package:note_taker_ui/infrastructure/telemetry_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

final class TelemetryDashboardController extends ChangeNotifier {
  TelemetryDashboardController({required TelemetryBridge telemetryBridge})
      : _bridge = telemetryBridge;
  final TelemetryBridge _bridge;
  List<TelemetryDevice> devices = const <TelemetryDevice>[];
  Map<String, List<TelemetryMetric>> metrics =
      <String, List<TelemetryMetric>>{};
  TelemetrySummary? summary;
  bool isLive = false;
  String? errorMessage;
  Timer? _timer;
  Future<void> load() async {
    try {
      devices = await _bridge.getDevices();
      summary = await _bridge.getSummary();
      metrics = {
        for (final TelemetryDevice device in devices)
          device.deviceId: await _bridge.getDeviceTelemetry(device.deviceId)
      };
      isLive = true;
      errorMessage = null;
    } on Object {
      isLive = false;
      errorMessage = l10n.unableToRefreshTelemetry;
    }
    notifyListeners();
  }

  void startPolling() {
    load();
    _timer = Timer.periodic(const Duration(seconds: 10), (_) => load());
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }
}

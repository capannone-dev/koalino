// Renders a responsive Material 3 telemetry dashboard from controller-owned safe DTOs.

import 'package:fl_chart/fl_chart.dart';
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/telemetry_dashboard_controller.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

final class TelemetryDashboardScreen extends StatefulWidget {
  const TelemetryDashboardScreen({required this.controller, super.key});
  final TelemetryDashboardController controller;
  @override
  State<TelemetryDashboardScreen> createState() =>
      _TelemetryDashboardScreenState();
}

final class _TelemetryDashboardScreenState
    extends State<TelemetryDashboardScreen> {
  @override
  void initState() {
    super.initState();
    widget.controller.startPolling();
  }

  @override
  Widget build(BuildContext context) => Scaffold(
      appBar: AppBar(title: Text(l10n.telemetryDashboard), actions: <Widget>[
        Padding(
            padding: const EdgeInsets.all(16),
            child: Text(widget.controller.isLive ? l10n.live : l10n.offline))
      ]),
      body: AnimatedBuilder(
          animation: widget.controller,
          builder: (BuildContext context, Widget? child) {
            final controller = widget.controller;
            if (controller.summary == null) {
              return const Center(child: CircularProgressIndicator());
            }
            final summary = controller.summary!;
            final allMetrics = controller.metrics.values
                .expand((List value) => value)
                .toList();
            return LayoutBuilder(
                builder: (BuildContext context, BoxConstraints constraints) {
              final count = constraints.maxWidth > 900 ? 4 : 2;
              return ListView(
                  padding: const EdgeInsets.all(16),
                  children: <Widget>[
                    GridView.count(
                        crossAxisCount: count,
                        shrinkWrap: true,
                        physics: const NeverScrollableScrollPhysics(),
                        childAspectRatio: 2.2,
                        children: <Widget>[
                          Stat(
                              l10n.transcription,
                              l10n.minutesShort(
                                  (summary.transcriptionMs / 60000)
                                      .toStringAsFixed(1))),
                          Stat(l10n.latency,
                              '${(summary.latencyMs / 1000).toStringAsFixed(2)} s'),
                          Stat(l10n.tasks, '${summary.tasks}'),
                          Stat(l10n.power,
                              '${(summary.powerMwh / 1000).toStringAsFixed(2)} Wh')
                        ]),
                    const SizedBox(height: 16),
                    Wrap(
                        spacing: 12,
                        runSpacing: 12,
                        children: controller.devices
                            .map((device) => Card(
                                child: Padding(
                                    padding: const EdgeInsets.all(12),
                                    child: Text(
                                        '${device.deviceId}\nVAD: ${device.supportsVad ? 'supported' : 'unavailable'}',
                                        style: TextStyle(
                                            color: _status(device.lastSeen))))))
                            .toList()),
                    const SizedBox(height: 16),
                    ChartCard(
                        title: l10n.transcriptionDurationOverTime,
                        chart: LineChart(LineChartData(
                            lineBarsData: controller.metrics.entries
                                .map((entry) => LineChartBarData(
                                    spots: entry.value
                                        .asMap()
                                        .entries
                                        .map((item) => FlSpot(
                                            item.key.toDouble(),
                                            item.value.durationMs.toDouble()))
                                        .toList()))
                                .toList()))),
                    ChartCard(
                        title: l10n.latencyDistribution,
                        chart: BarChart(BarChartData(
                            barGroups: <int>[0, 100, 500, 1000]
                                .asMap()
                                .entries
                                .map((entry) => BarChartGroupData(
                                        x: entry.key,
                                        barRods: <BarChartRodData>[
                                          BarChartRodData(
                                              toY: allMetrics
                                                  .where((metric) =>
                                                      metric.latencyMs >=
                                                      entry.value)
                                                  .length
                                                  .toDouble())
                                        ]))
                                .toList()))),
                    ChartCard(
                        title: l10n.taskTypeBreakdown,
                        chart: PieChart(
                            PieChartData(sections: _tasks(allMetrics)))),
                    ChartCard(
                        title: l10n.powerConsumptionEstimate,
                        chart: BarChart(BarChartData(
                            barGroups: controller.metrics.entries
                                .toList()
                                .asMap()
                                .entries
                                .map((entry) => BarChartGroupData(
                                        x: entry.key,
                                        barRods: <BarChartRodData>[
                                          BarChartRodData(
                                              toY: entry.value.value
                                                  .fold<double>(
                                                      0,
                                                      (double total, metric) =>
                                                          total +
                                                          metric.powerMwh))
                                        ]))
                                .toList())))
                  ]);
            });
          }));
}

final class Stat extends StatelessWidget {
  const Stat(this.label, this.value, {super.key});
  final String label;
  final String value;
  @override
  Widget build(BuildContext context) => Card(
      child: Padding(
          padding: const EdgeInsets.all(12),
          child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: <Widget>[
                Text(label),
                Text(value, style: Theme.of(context).textTheme.headlineSmall)
              ])));
}

final class ChartCard extends StatelessWidget {
  const ChartCard({required this.title, required this.chart, super.key});
  final String title;
  final Widget chart;
  @override
  Widget build(BuildContext context) => Card(
      child: SizedBox(
          height: 280,
          child: Padding(
              padding: const EdgeInsets.all(16),
              child: Column(
                  children: <Widget>[Text(title), Expanded(child: chart)]))));
}

Color _status(int timestamp) {
  final int age = DateTime.now().millisecondsSinceEpoch - timestamp;
  return age <= 300000
      ? Colors.green
      : age <= 1800000
          ? Colors.amber
          : Colors.red;
}

List<PieChartSectionData> _tasks(List<dynamic> metrics) {
  final Map<String, int> grouped = <String, int>{};
  for (final metric in metrics) {
    grouped[metric.taskType] = (grouped[metric.taskType] ?? 0) + 1;
  }
  return grouped.entries
      .map((entry) =>
          PieChartSectionData(value: entry.value.toDouble(), title: entry.key))
      .toList();
}

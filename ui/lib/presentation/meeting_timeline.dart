// Places a meeting's attachments among its transcript segments (Issue #54): a photo taken during
// the recording appears where the conversation was at that moment, and anything added after the
// meeting appears at the end.

import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';

/// One row of a meeting's transcript: a segment or an attachment.
sealed class MeetingTimelineEntry {
  const MeetingTimelineEntry();
}

/// A transcript segment.
final class SegmentEntry extends MeetingTimelineEntry {
  /// Wraps `segment`.
  const SegmentEntry(this.segment);

  final MeetingTranscriptSegment segment;
}

/// An attached picture or document.
final class AttachmentEntry extends MeetingTimelineEntry {
  /// Wraps `attachment`.
  const AttachmentEntry(this.attachment);

  final MeetingAttachment attachment;
}

/// Merges segments and attachments into one list.
///
/// Inputs: The segments in timeline order and the attachments in the order they were added.
/// Outputs: Each timed attachment follows every segment that started at or before its meeting
/// time (attachments at the same point keep their order); untimed ones come last.
/// Side effects: None.
List<MeetingTimelineEntry> buildMeetingTimeline({
  required List<MeetingTranscriptSegment> segments,
  required List<MeetingAttachment> attachments,
}) {
  final List<MeetingAttachment> timedAttachments = attachments
      .where((MeetingAttachment attachment) => attachment.meetingTimeMs != null)
      .toList();
  // A stable sort, so attachments taken at the same moment stay in the order they were added.
  _sortByTime(timedAttachments);
  final List<MeetingTimelineEntry> timeline = <MeetingTimelineEntry>[];
  int nextAttachment = 0;
  for (final MeetingTranscriptSegment segment in segments) {
    while (nextAttachment < timedAttachments.length &&
        timedAttachments[nextAttachment].meetingTimeMs! < segment.startTimeMs) {
      timeline.add(AttachmentEntry(timedAttachments[nextAttachment]));
      nextAttachment += 1;
    }
    timeline.add(SegmentEntry(segment));
  }
  timeline
    ..addAll(timedAttachments
        .skip(nextAttachment)
        .map((MeetingAttachment attachment) => AttachmentEntry(attachment)))
    ..addAll(attachments
        .where(
            (MeetingAttachment attachment) => attachment.meetingTimeMs == null)
        .map((MeetingAttachment attachment) => AttachmentEntry(attachment)));
  return timeline;
}

/// Sorts timed attachments by meeting time, keeping equal times in their original order.
///
/// Inputs: Attachments that all have a meeting time. Side effects: Reorders `attachments`.
void _sortByTime(List<MeetingAttachment> attachments) {
  final List<(int, MeetingAttachment)> indexed = <(int, MeetingAttachment)>[
    for (int index = 0; index < attachments.length; index += 1)
      (index, attachments[index]),
  ]..sort(((int, MeetingAttachment) left, (int, MeetingAttachment) right) {
      final int byTime =
          left.$2.meetingTimeMs!.compareTo(right.$2.meetingTimeMs!);
      return byTime != 0 ? byTime : left.$1.compareTo(right.$1);
    });
  attachments
    ..clear()
    ..addAll(indexed.map(((int, MeetingAttachment) entry) => entry.$2));
}

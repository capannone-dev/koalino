// Renders one meeting's renamable title, when it was created and the languages it was transcribed
// in, the devices it was recorded on with a Split action (#88), its project (Issue #85), its AI
// summary (#95), notes and editable transcript (#96), a chat about it (#97), and metadata-only
// attachment list from Axum.

import { ChatPanel } from "@/components/chat-panel";
import { MeetingTitleHeader } from "@/components/meeting-title-header";
import { MeetingProjectPicker } from "@/components/meeting-project-picker";
import { MeetingRecordedOn } from "@/components/meeting-recorded-on";
import { MeetingNotes } from "@/components/meeting-notes";
import { MeetingSummaryPanel } from "@/components/meeting-summary-panel";
import { MeetingTranscript } from "@/components/meeting-transcript";
import { getMeeting, getMeetings } from "@/lib/api";
import { describeTranscriptionLanguages, formatMeetingCreatedAt } from "@/lib/meeting-metadata";
import { getMeetingNotes } from "@/lib/edits-api";
import { getLlmSettings, getMeetingSummary } from "@/lib/llm-api";
import { getProjects, type ProjectSummary } from "@/lib/projects-api";

export default async function MeetingDetailPage({ params }: { params: Promise<{ meetingId: string }> }) {
  const { meetingId } = await params;
  const meeting = await getMeeting(meetingId);
  // The project picker is extra: an older daemon without projects still shows the meeting.
  const [projects, summaries, llmSettings, summaryState, notes] = await Promise.all([
    getProjects().catch((): ProjectSummary[] | null => null),
    getMeetings().catch(() => []),
    getLlmSettings().catch(() => null),
    getMeetingSummary(meetingId).catch(() => ({ summary: null, meeting_prompt: null })),
    getMeetingNotes(meetingId).catch(() => null)
  ]);
  const languages = describeTranscriptionLanguages(meeting.transcription_languages);
  const projectId = summaries.find((summary) => summary.meeting_id === meetingId)?.project_id ?? null;
  return (
    <section>
      <MeetingTitleHeader
        meeting={{ meeting_id: meeting.meeting_id, title: meeting.title, created_at: meeting.created_at }}
      />
      {projects === null ? null : <MeetingProjectPicker meetingId={meeting.meeting_id} projectId={projectId} projects={projects} />}
      <p className="text-muted">
        <time dateTime={meeting.created_at}>{formatMeetingCreatedAt(meeting.created_at)}</time>
      </p>
      {languages === null ? null : <p className="text-muted">{languages}</p>}
      <MeetingRecordedOn meeting={meeting} />
      <p>Meeting {meeting.meeting_id}</p>
      <MeetingSummaryPanel
        meetingId={meeting.meeting_id}
        summary={summaryState.summary}
        meetingPrompt={summaryState.meeting_prompt}
        connections={llmSettings?.connections ?? null}
        defaultTemplate={llmSettings?.template ?? ""}
      />
      <MeetingNotes meetingId={meeting.meeting_id} notes={notes} />
      <ChatPanel topic={{ scope: "meeting", id: meeting.meeting_id }} heading="Chat about this meeting" />
      <h2>Transcript</h2>
      <MeetingTranscript meetingId={meeting.meeting_id} segments={meeting.segments} transcript={meeting.transcript} />
      <h2>Attachments</h2>
      <ul>
        {meeting.attachments.map((attachment) => (
          <li key={attachment.attachment_id}>{attachment.file_path} ({attachment.mime_type})</li>
        ))}
      </ul>
    </section>
  );
}

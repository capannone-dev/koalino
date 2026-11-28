// Renders one meeting: a header with its renamable title, when it was created and the languages it
// was transcribed in, the devices it was recorded on with a Split action (#88), its project (#85)
// and Ask AI, a chat about it (#97); then the tabs (#201) Summary (#95), Transcript (#96), Notes
// (#96) and Files, with the kept recording's player (#140, #197) docked under every tab.

import { ChatPanel } from "@/components/chat-panel";
import { MeetingAskAi, MeetingTabs } from "@/components/meeting-tabs";
import { MeetingTitleHeader } from "@/components/meeting-title-header";
import { MeetingProjectPicker } from "@/components/meeting-project-picker";
import { MeetingRecordedOn } from "@/components/meeting-recorded-on";
import { MeetingRecordingPlayer } from "@/components/meeting-recording-player";
import { MeetingNotes } from "@/components/meeting-notes";
import { MeetingSummaryPanel } from "@/components/meeting-summary-panel";
import { MeetingTranscript } from "@/components/meeting-transcript";
import { RecordingPlaybackProvider } from "@/lib/recording-playback";
import { getMeeting, getMeetings } from "@/lib/api";
import { describeTranscriptionLanguages } from "@/lib/meeting-metadata";
import { MeetingTime } from "@/components/meeting-time";
import { getMeetingNotes } from "@/lib/edits-api";
import { getLlmSettings, getMeetingSummary } from "@/lib/llm-api";
import { getProjects, type ProjectSummary } from "@/lib/projects-api";
import { getT } from "@/lib/i18n/server";

export default async function MeetingDetailPage({ params }: { params: Promise<{ meetingId: string }> }) {
  const t = await getT();
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
  const languages = describeTranscriptionLanguages(meeting.transcription_languages, t);
  const projectId = summaries.find((summary) => summary.meeting_id === meetingId)?.project_id ?? null;
  return (
    <section>
      <MeetingTitleHeader
        meeting={{ meeting_id: meeting.meeting_id, title: meeting.title, created_at: meeting.created_at }}
      />
      {projects === null ? null : <MeetingProjectPicker meetingId={meeting.meeting_id} projectId={projectId} projects={projects} />}
      <p className="text-muted">
        <MeetingTime createdAt={meeting.created_at} />
      </p>
      {languages === null ? null : <p className="text-muted">{languages}</p>}
      <MeetingRecordedOn meeting={meeting} />
      <MeetingAskAi>
        <ChatPanel topic={{ scope: "meeting", id: meeting.meeting_id }} heading={t("chatAboutThisMeeting")} />
      </MeetingAskAi>
      <RecordingPlaybackProvider meetingId={meeting.meeting_id}>
        <MeetingTabs
          // A meeting with a summary opens on it; one without opens on its transcript.
          initial={summaryState.summary ? "summary" : "transcript"}
          panels={{
            summary: (
              <MeetingSummaryPanel
                meetingId={meeting.meeting_id}
                summary={summaryState.summary}
                meetingPrompt={summaryState.meeting_prompt}
                connections={llmSettings?.connections ?? null}
                defaultTemplate={llmSettings?.template ?? ""}
              />
            ),
            transcript: (
              <MeetingTranscript meetingId={meeting.meeting_id} segments={meeting.segments} transcript={meeting.transcript} />
            ),
            notes: <MeetingNotes meetingId={meeting.meeting_id} notes={notes} />,
            files:
              meeting.attachments.length === 0 ? (
                <p className="text-muted">{t("noFilesYet")}</p>
              ) : (
                <ul className="space-y-2">
                  {meeting.attachments.map((attachment) => (
                    <li key={attachment.attachment_id}>
                      {/* The file's name, not its path on the recording device (#211). */}
                      <span className="font-medium">{attachment.file_path.split(/[\\/]/).pop()}</span>{" "}
                      <span className="text-sm text-muted">{attachment.mime_type}</span>
                    </li>
                  ))}
                </ul>
              )
          }}
        />
        {/* After the tabs, so it sticks to the bottom of the window under every tab. */}
        <MeetingRecordingPlayer title={meeting.title} />
      </RecordingPlaybackProvider>
    </section>
  );
}

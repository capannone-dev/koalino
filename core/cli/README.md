<!-- Documents the command-line driving adapter and its stable command contract. -->

# Command-line adapter

This directory will contain the `clap` command model and service dispatch used by the Linux CLI.
It parses terminal input, delegates to application services and repository ports, and formats only
safe human-readable output. It must not access SQLite, perform identity/vector policy, or retain
raw audio itself.

## Commands

All commands open `NOTE_TAKER_DATABASE_PATH` (default `note_taker.sqlite3`), matching the daemon.
Voice embeddings in it are encrypted. Set `NOTE_TAKER_ENCRYPTION_PASSPHRASE` to derive the key
from a passphrase; otherwise a random key is kept in the platform keychain (service `note-taker`,
account `encryption-key`). An existing `~/.note_taker/encryption.key` (or
`NOTE_TAKER_ENCRYPTION_KEY_DIR`) is migrated there on first start and deleted. Without a usable
keychain, or with `NOTE_TAKER_KEYCHAIN_ENABLED=false`, the key file is used. Keep using the same
choice: a database encrypted one way refuses to open the other way, and a lost passphrase or
random key makes its voice data unrecoverable.
The executable's own log file records `info` by default (REQUIREMENTS.md 6.1), while stdout shows
only `error` entries, so terminal output contains only command results; the global
`--log-level <level>` flag sets both sinks for one invocation. `log-level <directive>`
changes a running daemon's verbosity without a restart (REQUIREMENTS.md 6.1): it validates the
directive and writes it to `<database>.log-level`, which the daemon on the same database polls
every 2 seconds. A request written while no daemon runs is ignored at the next start, where
`RUST_LOG` decides.

```text
note-taker record start --input <file.wav|-> [--meeting-id <id>] [--title <title>] [--language <code>]
note-taker record stop|status
note-taker meetings list
note-taker meetings view <meeting-id>
note-taker meetings rename <meeting-id> <new-title>
note-taker meetings delete <meeting-id> --yes
note-taker meetings split <meeting-id>
note-taker voice delete-cluster <cluster-id> --yes
note-taker voice delete-person <identity-id> --yes
note-taker meetings export <meeting-id> --format <markdown|pdf|json> [--output <path>] [--no-attachments] [--no-metadata]
note-taker meetings export-all --format <markdown|pdf|json> [--date-range <YYYY-MM-DD>:<YYYY-MM-DD>] [--output <path>] [--no-attachments] [--no-metadata]
note-taker voice list
note-taker voice assign <clip-uuid> <identity-uuid>
note-taker voice list-clusters <meeting-id>
note-taker voice assign-cluster <cluster-uuid> <identity-name>
note-taker devices list
note-taker devices revoke <peer-id|name>
note-taker telemetry
note-taker log-level <error|warn|info|debug|trace|directive>
note-taker glossary [show | set <term>... | add <term>... | remove <term>... | clear]
note-taker keep-speech [days]
note-taker summary <meeting-id> [--connection <id>] [--show] [--on-paired-computer] [--cancel-request]
note-taker search <word>... [--projects | --project <id>]
note-taker notes-folder [status | on [<path>] | off | sync]
note-taker chat <question>... [--meeting <id> | --project <id>] [--history | --clear]
note-taker ai [list | add --label L --kind openai --url U --model M [--context N] | remove <id> | default <id> | test <id> | auto on|off]
note-taker [--log-level <level>] <command> ...
```

`devices list` (Issue #43) prints `Peer ID | Name | Paired` for the devices this database syncs
with (pairing time in UTC), or `No paired devices.`. `devices revoke` removes one, selected by its
exact peer ID or by its name in any case; a name shared by several devices, or one that matches
nothing, is refused with exit status 1. Both open the allowlist table in the shared database (only
`devices` does, so other commands never create the P2P tables). A daemon running on the same
database compares its gate with that table every second, so it drops and disconnects a revoked
device without a restart. Pairing itself needs the code comparison on a running device, so it is
done from a Devices screen (app or web), not the CLI.

`record start --title` stores a title on the meeting (Issue #35). Without it, a new meeting is
untitled and an existing one keeps its title. Blank titles, titles longer than 200 characters,
and titles with control characters are rejected before recording starts.

`record start --language` sets the language spoken in this meeting: `auto` or a Whisper code such
as `it`. Without it, the recording uses `NOTE_TAKER_WHISPER_LANGUAGE` (else `auto`). Either way the
language used is stored in the meeting's details, and `meetings view` shows it as
`Transcription language: it`. An unknown code is refused before recording starts.

The delete commands share `application::deletion` with the HTTP API. They need `--yes`, because the
deletion cannot be undone and also reaches paired devices. A meeting being recorded is refused.

`meetings rename` (Issue #38) replaces a meeting's title through `application::meeting_rename`,
the same use case as the HTTP API and FFI. It prints
`Renamed meeting <id>: "<old title>" -> "<new title>"`, where the old title is the dated fallback
for a previously untitled meeting. The same title rules apply as for `--title`. An unknown meeting
prints `meeting <id> does not exist`, an invalid title prints `invalid title: <reason>`, and both
exit with status 1 without writing anything.

`meetings list` prints `ID | Title | Date | Duration | Devices`. Title is the stored title, or
`Meeting YYYY-MM-DD HH:MM UTC` for untitled meetings. Date is the UTC creation time. Duration is
the end of the latest transcript segment (`M:SS`, or `H:MM:SS` from one hour). Devices is how many
devices recorded the meeting: a meeting recorded on the laptop and on a paired phone (#88) is
listed once, under the earlier recording, with `2`. `meetings view` prints the title, underlined,
on its first line, followed by the ID, creation time, the devices line for a combined meeting,
transcript, and attachments, and reports unknown meetings as errors.

`meetings split <meeting-id>` lists the recordings combined into a meeting on their own again,
for good: the app never combines them again. Each shows its own live transcript; the meeting keeps
its transcript, which may already combine every device's speech. Passing a combined recording's
ID splits only that one. `meetings delete` on a combined meeting also deletes the recordings
combined into it. `voice assign`
finds the clip through `NoteRepository` and delegates signature storage to `VoiceIdentityManager`;
it never performs vector matching or SQL in the CLI.

### Unknown-speaker clusters

Both commands call `application::cluster_review`, the same use case as the HTTP API and FFI.

`voice list-clusters <meeting-id>` prints the meeting's unresolved clusters. Durations are in
seconds, rounded to one decimal place:

```text
Cluster ID | Total Duration | Chunks
4b1c…-… | 12.5s | 3
```

A meeting with no unresolved clusters, including an unknown meeting ID, prints
`No unresolved unknown-speaker clusters for meeting <meeting-id>`.

`voice assign-cluster <cluster-uuid> "<identity name>"` resolves the name and assigns the cluster.
A known identity with the same name (trimmed, case-insensitive) is reused. Any other name gets a
new identity UUID. Every member embedding is stored as a voice variant, the cluster's transcript
segments are relinked, the cluster is resolved, and its review audio is deleted. It prints:

```text
Assigned cluster <cluster-uuid> to "Grace Hopper" (new identity <identity-uuid>; 3 voice variants stored, 4 transcript segments updated)
```

Blank names, unknown clusters, and already-resolved clusters exit non-zero with a readable message.

The CLI binary composes `SqliteVoiceVectorDB` on `NOTE_TAKER_DATABASE_PATH`, so the name and
the voice variants persist with the relinked segments and resolved cluster. A later
`voice list` shows the identity, and a later `voice assign-cluster` with the same name in any
letter case reuses it.

### Exports

`meetings export` and `meetings export-all` call the same `ExportManager` that the HTTP API and
the Flutter FFI use. The binary composes an `ExportWriter` with every exporter from
`adapters::export::default_exporters()` and the directory from `NOTE_TAKER_EXPORT_DIR` (default
`~/Documents/NoteTaker/exports`).

- `--format` accepts `markdown` (or `md`), `pdf`, and `json`, case-insensitively. `srt` is reserved
  and rejected with `srt export is not supported yet`.
- `--output` writes to an explicit file. Without it, a unique name such as
  `2026-10-27_<meeting-id>_20261027T091500123Z.md` is generated in the export directory.
- `--date-range 2026-10-01:2026-10-31` selects meetings created between the start day's midnight
  and the end day's last millisecond (UTC, inclusive). Without a range, `export-all` exports
  every meeting into one file.
- Markdown exports copy attachments into `<file stem>_attachments/` beside the file and link them
  relatively. PDF exports embed them. JSON exports list their original paths.

On success the CLI prints `Exported <n> meeting(s) as <format> to <path> (<bytes> bytes)`. Unknown
meetings, empty selections, and invalid flags exit non-zero with a readable message.

## Recording (Issue #32)

`record start` runs in the foreground. It composes the meeting pipeline (see
`core/bin/README.md` for the transcription variables), feeds 16 kHz mono 16-bit PCM from a WAV
file or, with `--input -`, raw little-endian PCM from stdin, and prints a summary once every
chunk is transcribed. A microphone can be recorded on Linux without extra dependencies:

```text
arecord -f S16_LE -r 16000 -c 1 -t raw | note-taker record start --input - --meeting-id standup
```

The recording ends when the input ends, on Ctrl+C, or when another terminal runs `record stop`.
`record stop` waits until the recording has transcribed its remaining audio. `record status`
prints `Recording (meeting …, pid …)` or `Idle`. Without a transcription engine, `record start`
fails with the variables to set. The stop request is noticed between 100 ms input blocks, so a
silent stdin pipe that delivers no data delays it.

## Transcription glossary

`glossary` shows or edits the names and terms that steer transcription, kept in
`<database>.transcription-settings.json` next to the apps' other transcription choices. Each
argument is one term (quote multi-word names). `remove` ignores case. A change applies from the
next recording and to the next after-meeting pass. An invalid list (a term over 60 characters,
or more than 500 characters in all) is refused and the saved one kept.

## Speech retention window

`keep-speech` shows how many days a recording's speech may wait to be combined with another
device's recording of the same meeting; `keep-speech <days>` changes it (1 to 60, three weeks by
default, #93). It is kept in the same settings file, and speech older than a shorter window is
deleted at the after-meeting worker's next round.

## Meeting participants' voices

`meeting-voices` shows whether an online-meeting participant's voice is kept in the voice gallery;
`meeting-voices on|off` switches it (#103). It is off by default because voice prints are
biometric. When on, a participant whose track carried one voice for about 30 s gets that voice in
the gallery, unless it sounds like someone else already there. A meeting room or other track with
several voices is never kept. It is kept in the same settings file and read at every offer.

`summary <meeting-id>` writes the meeting's summary with the default AI connection (or the one
`--connection` names) and prints it; the meeting's transcript, notes and photos are sent to that
connection's server. `--show` prints the stored summary instead, and how far a request to a
paired computer has got. `--on-paired-computer` asks a paired computer to write it with its own
connection (#107): the request goes into the meeting document, which syncs, and nothing is sent to
an AI server from this device; `--cancel-request` withdraws it. `ai` manages the connections
(#95): `add` saves an OpenAI-compatible server on this computer or the local network (Koalino
online services with an API key are added in the app or the web UI, #138); `test`
sends a one-line request holding no meeting content; `auto on|off` turns automatic summaries on or
off.

`search <word>...` (#100) prints where the words were said or written, one line per hit:
`[meeting ID] title · where: snippet`, with matched words in brackets (`1:01 Anna` for a
transcript passage, `summary`, `notes`, `title`, or a project file). It searches every meeting by
default, every project with `--projects`, or one with `--project <id>`. Case and accents are
ignored, and every word must be in the same passage.

`chat <question>... [--meeting <id> | --project <id>] [--connection <id>]` (#97) asks the
default AI connection about every meeting, summary and project, or about one meeting or
project, and prints the answer with what the model was given. `--history` prints the chat so
far and `--clear` deletes it. Chats are kept on this device, sealed.

`notes-folder [status | on [<path>] | off | sync]` (#98) shows or changes the notes folder: plain
files mirroring projects and meetings (see `bin/README.md`). `on` writes it at once, in `<path>`,
the saved folder or `~/NoteTaker`; `sync` reads edits back and writes it now. The daemon keeps it
up to date every 30 seconds while it runs.

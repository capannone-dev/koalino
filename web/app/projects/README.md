<!-- The Projects pages of the dashboard (Issue #85). -->

# Projects

A project is a folder of meetings with their files, notes and e-mails; the same projects appear
in the app's Projects tab and sync between paired devices.

- `page.tsx` (`/projects`) lists projects by name with their counts, from `GET /api/projects`,
  and creates one (`components/projects-index.tsx`), then opens it.
- `[projectId]/page.tsx` (`/projects/<id>`) renders `components/project-view.tsx` with the project
  and every meeting, so meetings outside any project can be added. The page renames and deletes
  the project (its meetings stay), removes meetings, uploads files (an `.eml` adds its attachments
  too), writes Markdown notes, and deletes single files.
- Previews stay inside the page and never render a file's markup: pictures inline, Markdown and
  text as plain text, e-mails as headers and text (Rust turns HTML bodies into text). PDFs open
  from the daemon in a new tab; every file can be downloaded. The daemon serves files with
  `nosniff` and a sandbox Content Security Policy.
- A file still syncing to the daemon's device shows "Syncing…" and can only be deleted.

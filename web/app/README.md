<!-- Documents App Router pages for the dashboard. -->

# App Router pages

This directory contains server-rendered page composition and global styles. Pages load data only
through `web/lib/api.ts`; interactive behavior is isolated in client components.

`error.tsx` is the route error boundary for every page. When a server page cannot reach the Axum
daemon, it explains that the note-taking server did not respond (without showing the raw error,
which can contain internal details) and offers **Try again**.

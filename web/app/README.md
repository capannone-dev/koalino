<!-- Documents App Router pages for the dashboard. -->

# App Router pages

This directory contains server-rendered page composition and global styles. Pages load data only
through `web/lib/api.ts`; interactive behavior is isolated in client components.

`layout.tsx` resolves the request's language (#124: the `koalino-language` cookie, else the browser's
`Accept-Language`), sets `<html lang dir>` (right to left for Arabic) and wraps every page in
`I18nProvider`. Server pages word their text with `await getT()`; every page therefore renders per
request.

`error.tsx` is the route error boundary for every page. When a server page cannot reach the Axum
daemon, it explains that the note-taking server did not respond (without showing the raw error,
which can contain internal details) and offers **Try again**.

`styles.css` defines the colour tokens (`canvas`, `surface`, `ink`, `muted`, `line`, `accent`,
`success`, `warning`, `danger`, …) in Tailwind's `@theme`, with dark values under
`prefers-color-scheme: dark`. Components use only these tokens (`bg-surface`, `text-muted`), so
every page follows the browser's light or dark setting. Recharts' fixed SVG colours are
overridden there too.

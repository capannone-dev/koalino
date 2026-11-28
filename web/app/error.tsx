// Catches failures of any dashboard route (usually an unreachable daemon) and offers a retry, in
// plain words, with the technical hint folded under Details (#203).

"use client";

import { useT } from "@/lib/i18n/client";

/**
 * Route error boundary for every page under `app/`.
 *
 * Inputs: the thrown `error` (its message is not shown, because it can contain internal paths or
 * server details) and `reset`, which re-renders the route. Outputs: a short explanation and a
 * Try again button. Side effects: none beyond calling `reset` on click.
 */
export default function RouteError({ reset }: { error: Error & { digest?: string }; reset: () => void }) {
  const t = useT();
  return (
    <section role="alert" className="mx-auto max-w-xl space-y-3 rounded-xl border border-danger-line bg-surface p-6">
      <h1 className="text-xl font-bold">{t("cantLoadThisPage")}</h1>
      <p className="text-ink-soft">{t("koalinoIsNotResponding")}</p>
      {/* The technical hint is for whoever runs the server, folded away for everyone else (#203). */}
      <details className="text-sm text-muted">
        <summary className="cursor-pointer">{t("detailsForTheAdministrator")}</summary>
        <p className="mt-1">{t("theNoteTakingServerDidnt")}</p>
      </details>
      <button
        type="button"
        onClick={reset}
        className="min-h-11 rounded-lg bg-accent px-4 font-medium text-on-accent hover:bg-accent-strong"
      >{t("tryAgain")}</button>
    </section>
  );
}

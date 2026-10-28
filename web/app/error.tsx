// Catches failures of any dashboard route (usually an unreachable daemon) and offers a retry.

"use client";

/**
 * Route error boundary for every page under `app/`.
 *
 * Inputs: the thrown `error` (its message is not shown, because it can contain internal paths or
 * server details) and `reset`, which re-renders the route. Outputs: a short explanation and a
 * Try again button. Side effects: none beyond calling `reset` on click.
 */
export default function RouteError({ reset }: { error: Error & { digest?: string }; reset: () => void }) {
  return (
    <section role="alert" className="mx-auto max-w-xl space-y-3 rounded-xl border border-red-200 bg-white p-6">
      <h1 className="text-xl font-bold">Can&apos;t load this page</h1>
      <p className="text-slate-700">
        The note-taking server didn&apos;t respond. Check that the daemon is running and that
        NEXT_PUBLIC_API_BASE_URL points to it, then try again.
      </p>
      <button
        type="button"
        onClick={reset}
        className="min-h-11 rounded-lg bg-blue-700 px-4 font-medium text-white hover:bg-blue-800"
      >
        Try again
      </button>
    </section>
  );
}

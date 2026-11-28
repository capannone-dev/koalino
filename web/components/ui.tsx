// The dashboard's shared building blocks (#207), so every page uses the same controls as the app:
// filled / tonal / outlined / text buttons (one filled button per screen for the main action), the
// text field, the empty state (icon, what is missing, the next step) and the banner.

import type { ReactNode } from "react";
import { Icon, type IconName } from "@/components/icons";

const buttonBase =
  "inline-flex min-h-11 items-center justify-center gap-2 rounded-lg px-4 font-medium focus-visible:outline-2 focus-visible:outline-focus";

/**
 * Class names of the button kinds, as in Material 3: `filled` for the screen's main action,
 * `tonal` for a strong secondary one, `outlined` for the others, `text` for the quietest.
 */
export const buttonStyles = {
  filled: `${buttonBase} bg-accent text-on-accent hover:bg-accent-strong disabled:bg-disabled`,
  tonal: `${buttonBase} bg-accent-soft text-accent-text hover:bg-surface-muted disabled:text-faint`,
  outlined: `${buttonBase} border border-line-strong hover:bg-surface-muted disabled:text-faint`,
  text: `${buttonBase} text-accent-text hover:bg-accent-soft disabled:text-faint`,
} as const;

/** The class names of a full-width text field. */
export const fieldStyle =
  "mt-1 block min-h-11 w-full rounded-lg border border-line-strong px-3 text-base focus:border-focus focus:outline-none focus:ring-2 focus:ring-focus/30 aria-invalid:border-danger";

/**
 * Says that a list is empty and what to do next.
 *
 * Inputs: an icon, the title, an optional explanation (`children`) and an optional action (a
 * link or button, usually styled `buttonStyles.filled`). Outputs: a centred card. Side effects: none.
 */
export function EmptyState({
  icon,
  title,
  children,
  action,
}: {
  icon: IconName;
  title: string;
  children?: ReactNode;
  action?: ReactNode;
}) {
  return (
    <div className="flex flex-col items-center rounded-xl border border-line bg-surface p-6 text-center">
      <Icon name={icon} size={40} className="text-muted" />
      <p className="mt-2 font-semibold">{title}</p>
      {children ? <div className="mt-1 text-muted">{children}</div> : null}
      {action ? <div className="mt-4">{action}</div> : null}
    </div>
  );
}

const bannerTones = {
  danger: { icon: "warning", className: "border-danger-line text-danger", role: "alert" },
  warning: { icon: "warning", className: "border-line-strong text-warning", role: "status" },
  info: { icon: "info", className: "border-line text-accent-text", role: "status" },
} as const satisfies Record<string, { icon: IconName; className: string; role: string }>;

/**
 * A notice at the top of a page or section: an icon, a title, the explanation and an action.
 *
 * Inputs: the tone (`danger` is announced as an alert), the title, `children` and an optional
 * action. Outputs: a section. Side effects: none.
 */
export function Banner({
  tone,
  title,
  children,
  action,
  className = "",
}: {
  tone: keyof typeof bannerTones;
  title: string;
  children?: ReactNode;
  action?: ReactNode;
  className?: string;
}) {
  const style = bannerTones[tone];
  return (
    <section role={style.role} className={`flex gap-3 rounded-xl border bg-surface p-4 ${style.className.split(" ")[0]} ${className}`}>
      <Icon name={style.icon} className={`mt-0.5 shrink-0 ${style.className.split(" ")[1]}`} />
      <div className="min-w-0 flex-1 space-y-2 text-ink">
        <h2 className="text-lg font-bold">{title}</h2>
        {children}
        {action ? <div>{action}</div> : null}
      </div>
    </section>
  );
}

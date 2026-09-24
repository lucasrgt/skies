import type { ReactNode } from "react";
import { color, text, type TextRole } from "./theme";

// A role maps to the document outline (one title per screen);
// size + line-height + weight travel together so typography is one decision, not three.
const TAG: Record<TextRole, "h1" | "h2" | "p" | "span"> = {
  display: "h1",
  title: "h1",
  heading: "h2",
  body: "p",
  label: "span",
  caption: "span",
};

export function Text({
  children,
  role = "body",
  tone = "default",
  alert = false,
}: {
  children: ReactNode;
  role?: TextRole;
  tone?: "default" | "muted" | "danger" | "inverse";
  // Announces the text to assistive tech (role="alert") — the command-error surface a form renders
  // above its submit.
  alert?: boolean;
}) {
  const Tag = TAG[role];
  const t = text[role];
  const TONE = { default: color.text, muted: color.textMuted, danger: color.danger, inverse: color.textInverse };
  return (
    <Tag
      role={alert ? "alert" : undefined}
      style={{
        // Margins belong to the container (Stack gap), so the element defaults are zeroed.
        margin: 0,
        fontSize: t.fontSize,
        lineHeight: t.lineHeight + "px",
        fontWeight: t.fontWeight,
        color: TONE[tone],
      }}
    >
      {children}
    </Tag>
  );
}

// The kit's style constants. Plain app code: change the values freely.

export type Space = "none" | "xs" | "sm" | "md" | "lg" | "xl";
export type TextRole = "display" | "title" | "heading" | "body" | "label" | "caption";

/** Spacing on a 4px grid. */
export const space: Record<Space, number> = { none: 0, xs: 4, sm: 8, md: 12, lg: 16, xl: 24 };

export const radius = { md: 8, lg: 12 } as const;

export const text: Record<TextRole, { fontSize: number; lineHeight: number; fontWeight: 400 | 500 | 600 | 700 }> = {
  caption: { fontSize: 12, lineHeight: 16, fontWeight: 400 },
  label: { fontSize: 14, lineHeight: 20, fontWeight: 500 },
  body: { fontSize: 16, lineHeight: 24, fontWeight: 400 },
  heading: { fontSize: 20, lineHeight: 28, fontWeight: 600 },
  title: { fontSize: 25, lineHeight: 32, fontWeight: 600 },
  display: { fontSize: 31, lineHeight: 40, fontWeight: 700 },
};

export const color = {
  bg: "#f8fafc",
  surface: "#ffffff",
  border: "#e2e8f0",
  borderStrong: "#cbd5e1",
  text: "#0f172a",
  textMuted: "#64748b",
  textInverse: "#ffffff",
  primary: "#2563eb",
  primaryHover: "#1d4ed8",
  primaryActive: "#1e40af",
  onPrimary: "#ffffff",
  danger: "#dc2626",
  dangerHover: "#b91c1c",
  onDanger: "#ffffff",
  focusRing: "#2563eb",
} as const;

export const shadow = { raised: "0 1px 3px rgba(0,0,0,0.12)" } as const;

export const motionMs = { fast: 100 } as const;

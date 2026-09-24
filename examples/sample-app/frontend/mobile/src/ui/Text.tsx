import type { ReactNode } from "react";
import { Text as RNText } from "react-native";
import { color, text, type TextRole } from "./theme";

export function Text({
  children,
  role = "body",
  tone = "default",
  alert = false,
}: {
  children: ReactNode;
  role?: TextRole;
  tone?: "default" | "muted" | "danger" | "inverse";
  alert?: boolean;
}) {
  const t = text[role];
  const TONE = { default: color.text, muted: color.textMuted, danger: color.danger, inverse: color.textInverse };
  return (
    <RNText
      accessibilityRole={role === "display" || role === "title" || role === "heading" ? "header" : undefined}
      accessibilityLiveRegion={alert ? "polite" : undefined}
      style={{
        fontSize: t.fontSize,
        lineHeight: t.lineHeight,
        fontWeight: String(t.fontWeight) as "400" | "500" | "600" | "700",
        color: TONE[tone],
      }}
    >
      {children}
    </RNText>
  );
}

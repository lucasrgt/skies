'use client';

// Stateful kit primitive → a client component: the Next App Router needs the directive; a no-op on Vite.
import { useState } from "react";
import { color, motionMs, radius, space, text } from "./theme";

type Variant = "primary" | "secondary" | "danger";

// Every variant carries its full state set; a consumer can't forget a state because it never
// implements one.
const FILL: Record<Variant, { rest: string; hover: string; active: string; on: string }> = {
  primary: { rest: color.primary, hover: color.primaryHover, active: color.primaryActive, on: color.onPrimary },
  secondary: { rest: color.surface, hover: color.bg, active: color.border, on: color.text },
  danger: { rest: color.danger, hover: color.dangerHover, active: color.dangerHover, on: color.onDanger },
};

// A button is a labeled action — `label` is a string by contract, so the accessible name is the
// visible name. `loading` blocks and announces; the focus ring is the replacement for the outline
// it suppresses (never removed without one).
export function Button({
  label,
  onClick,
  variant = "primary",
  disabled = false,
  loading = false,
}: {
  label: string;
  onClick: () => void;
  variant?: Variant;
  disabled?: boolean;
  loading?: boolean;
}) {
  const [hover, setHover] = useState(false);
  const [active, setActive] = useState(false);
  const [focused, setFocused] = useState(false);
  const blocked = disabled || loading;
  const fill = FILL[variant];
  const background = blocked ? color.border : active ? fill.active : hover ? fill.hover : fill.rest;

  return (
    <button
      type="button"
      disabled={blocked}
      aria-busy={loading || undefined}
      onClick={onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => {
        setHover(false);
        setActive(false);
      }}
      onMouseDown={() => setActive(true)}
      onMouseUp={() => setActive(false)}
      onFocus={() => setFocused(true)}
      onBlur={() => setFocused(false)}
      style={{
        minHeight: 44,
        paddingLeft: space.lg,
        paddingRight: space.lg,
        borderRadius: radius.md,
        borderWidth: 1,
        borderStyle: "solid",
        borderColor: variant === "secondary" ? color.borderStrong : "transparent",
        backgroundColor: background,
        color: blocked ? color.textMuted : fill.on,
        fontSize: text.label.fontSize,
        lineHeight: text.label.lineHeight + "px",
        fontWeight: text.label.fontWeight,
        cursor: blocked ? "not-allowed" : "pointer",
        transition: "background-color " + motionMs.fast + "ms ease",
        outline: "none",
        boxShadow: focused ? "0 0 0 2px " + color.focusRing : "none",
      }}
    >
      {label}
    </button>
  );
}

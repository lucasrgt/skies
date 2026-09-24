'use client';

// Stateful kit primitive → a client component: the Next App Router needs the directive; a no-op on Vite/RN.
import { createContext, useContext, type ReactNode } from "react";
import { color, space, text } from "./theme";

// Field → control wiring travels through kit-internal context, so the anatomy (label → control →
// hint|error) and the aria plumbing are one decision made here, never re-made per screen.
interface FieldWiring {
  describedBy?: string;
  invalid?: boolean;
}
const FieldContext = createContext<FieldWiring>({});

/** Kit-internal: the control (Input) reads its aria wiring from the enclosing Field. */
export function useFieldWiring(): FieldWiring {
  return useContext(FieldContext);
}

export function Field({
  fieldId,
  label,
  hint,
  error,
  children,
}: {
  fieldId: string;
  label: string;
  hint?: string;
  error?: string;
  children: ReactNode;
}) {
  const messageId = error ? fieldId + "-error" : hint ? fieldId + "-hint" : undefined;
  const message = {
    fontSize: text.caption.fontSize,
    lineHeight: text.caption.lineHeight + "px",
    fontWeight: text.caption.fontWeight,
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: space.xs }}>
      <label
        htmlFor={fieldId}
        style={{
          fontSize: text.label.fontSize,
          lineHeight: text.label.lineHeight + "px",
          fontWeight: text.label.fontWeight,
          color: color.text,
        }}
      >
        {label}
      </label>
      <FieldContext.Provider value={{ describedBy: messageId, invalid: Boolean(error) }}>
        {children}
      </FieldContext.Provider>
      {error ? (
        <span id={fieldId + "-error"} role="alert" style={{ ...message, color: color.danger }}>
          {error}
        </span>
      ) : hint ? (
        <span id={fieldId + "-hint"} style={{ ...message, color: color.textMuted }}>
          {hint}
        </span>
      ) : null}
    </div>
  );
}

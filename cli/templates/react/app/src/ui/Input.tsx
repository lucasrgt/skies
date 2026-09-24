'use client';

// Stateful kit primitive → a client component: the Next App Router needs the directive; a no-op on Vite.
import { useState, type ChangeEvent } from "react";
import { color, radius, space, text } from "./theme";
import { useFieldWiring } from "./Field";

// A web input: `onChange` receives the DOM change event, so a react-hook-form `field.onChange` (and `field.onBlur`)
// binds straight to it. The enclosing Field supplies the aria wiring; an explicit `invalid` prop wins over it.
export function Input({
  id,
  name,
  value,
  onChange,
  onBlur,
  placeholder,
  kind = "text",
  invalid,
}: {
  id: string;
  name?: string;
  value: string;
  onChange: (event: ChangeEvent<HTMLInputElement>) => void;
  onBlur?: () => void;
  placeholder?: string;
  kind?: "text" | "email" | "password" | "number";
  invalid?: boolean;
}) {
  const wiring = useFieldWiring();
  const [focused, setFocused] = useState(false);
  const isInvalid = invalid ?? wiring.invalid ?? false;

  return (
    <input
      id={id}
      name={name}
      type={kind === "number" ? "text" : kind}
      inputMode={kind === "number" ? "numeric" : kind === "email" ? "email" : undefined}
      value={value}
      onChange={onChange}
      placeholder={placeholder}
      aria-invalid={isInvalid || undefined}
      aria-describedby={wiring.describedBy}
      onFocus={() => setFocused(true)}
      onBlur={() => {
        setFocused(false);
        onBlur?.();
      }}
      style={{
        minHeight: 44,
        paddingLeft: space.md,
        paddingRight: space.md,
        borderRadius: radius.md,
        borderWidth: 1,
        borderStyle: "solid",
        borderColor: isInvalid ? color.danger : color.borderStrong,
        backgroundColor: color.surface,
        color: color.text,
        fontSize: text.body.fontSize,
        lineHeight: text.body.lineHeight + "px",
        outline: "none",
        boxShadow: focused ? "0 0 0 2px " + color.focusRing : "none",
      }}
    />
  );
}

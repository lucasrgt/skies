import { createContext, useContext, type ReactNode } from "react";
import { View, Text as RNText } from "react-native";
import { color, space, text } from "./theme";

// RN has no htmlFor; the label travels to the control through kit-internal context so the
// anatomy (label → control → hint|error) stays one decision, like the web Field.
interface FieldWiring {
  label?: string;
  invalid?: boolean;
}
const FieldContext = createContext<FieldWiring>({});

/** Kit-internal: the control (Input) reads its accessibility wiring from the enclosing Field. */
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
  const message = { fontSize: text.caption.fontSize, lineHeight: text.caption.lineHeight };
  return (
    <View nativeID={fieldId} style={{ gap: space.xs }}>
      <RNText
        style={{
          fontSize: text.label.fontSize,
          lineHeight: text.label.lineHeight,
          fontWeight: String(text.label.fontWeight) as "500",
          color: color.text,
        }}
      >
        {label}
      </RNText>
      <FieldContext.Provider value={{ label, invalid: Boolean(error) }}>{children}</FieldContext.Provider>
      {error ? (
        <RNText accessibilityLiveRegion="polite" style={{ ...message, color: color.danger }}>
          {error}
        </RNText>
      ) : hint ? (
        <RNText style={{ ...message, color: color.textMuted }}>{hint}</RNText>
      ) : null}
    </View>
  );
}

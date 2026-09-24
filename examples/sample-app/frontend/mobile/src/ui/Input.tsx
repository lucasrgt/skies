import { TextInput } from "react-native";
import { color, radius, space, text } from "./theme";
import { useFieldWiring } from "./Field";

export function Input({
  id,
  value,
  onChangeText,
  placeholder,
  kind = "text",
  invalid,
}: {
  id: string;
  value: string;
  onChangeText: (v: string) => void;
  placeholder?: string;
  kind?: "text" | "email" | "password" | "number";
  invalid?: boolean;
}) {
  const wiring = useFieldWiring();
  const isInvalid = invalid ?? wiring.invalid ?? false;
  return (
    <TextInput
      nativeID={id}
      accessibilityLabel={wiring.label}
      keyboardType={kind === "number" ? "numeric" : kind === "email" ? "email-address" : "default"}
      secureTextEntry={kind === "password"}
      value={value}
      onChangeText={onChangeText}
      placeholder={placeholder}
      placeholderTextColor={color.textMuted}
      style={{
        minHeight: 44,
        paddingHorizontal: space.md,
        borderRadius: radius.md,
        borderWidth: 1,
        borderColor: isInvalid ? color.danger : color.borderStrong,
        backgroundColor: color.surface,
        color: color.text,
        fontSize: text.body.fontSize,
      }}
    />
  );
}

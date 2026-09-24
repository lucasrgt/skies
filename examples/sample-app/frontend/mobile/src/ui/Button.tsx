import { Pressable, Text as RNText } from "react-native";
import { color, radius, space, text } from "./theme";

type Variant = "primary" | "secondary" | "danger";

const FILL: Record<Variant, { rest: string; pressed: string; on: string }> = {
  primary: { rest: color.primary, pressed: color.primaryActive, on: color.onPrimary },
  secondary: { rest: color.surface, pressed: color.border, on: color.text },
  danger: { rest: color.danger, pressed: color.dangerHover, on: color.onDanger },
};

export function Button({
  label,
  onPress,
  variant = "primary",
  disabled = false,
  loading = false,
}: {
  label: string;
  onPress: () => void;
  variant?: Variant;
  disabled?: boolean;
  loading?: boolean;
}) {
  const blocked = disabled || loading;
  const fill = FILL[variant];
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityState={{ disabled: blocked, busy: loading }}
      disabled={blocked}
      onPress={onPress}
      style={({ pressed }) => ({
        minHeight: 44,
        paddingHorizontal: space.lg,
        borderRadius: radius.md,
        borderWidth: 1,
        borderColor: variant === "secondary" ? color.borderStrong : "transparent",
        backgroundColor: blocked ? color.border : pressed ? fill.pressed : fill.rest,
        alignItems: "center",
        justifyContent: "center",
      })}
    >
      <RNText
        style={{
          fontSize: text.label.fontSize,
          lineHeight: text.label.lineHeight,
          fontWeight: String(text.label.fontWeight) as "500",
          color: blocked ? color.textMuted : fill.on,
        }}
      >
        {label}
      </RNText>
    </Pressable>
  );
}

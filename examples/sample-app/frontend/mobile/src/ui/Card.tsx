import type { ReactNode } from "react";
import { View } from "react-native";
import { color, radius, space, type Space } from "./theme";

// The web shadow is a box-shadow string; RN expresses elevation natively — this is the map.
const ELEVATION = { none: 0, raised: 2, overlay: 8 } as const;

export function Card({
  children,
  padding = "lg",
  listItem = false,
}: {
  children: ReactNode;
  padding?: Space;
  listItem?: boolean;
}) {
  return (
    <View
      role={listItem ? "listitem" : undefined}
      style={{
        backgroundColor: color.surface,
        borderWidth: 1,
        borderColor: color.border,
        borderRadius: radius.lg,
        elevation: ELEVATION.raised,
        padding: space[padding],
      }}
    >
      {children}
    </View>
  );
}

import type { ReactNode } from "react";
import { View } from "react-native";
import { space, type Space } from "./theme";

const ALIGN = { start: "flex-start", center: "center", end: "flex-end", stretch: "stretch" } as const;

export function Stack({
  children,
  gap = "md",
  direction = "vertical",
  align,
  padding,
}: {
  children: ReactNode;
  gap?: Space;
  direction?: "vertical" | "horizontal";
  align?: "start" | "center" | "end" | "stretch";
  padding?: Space;
}) {
  return (
    <View
      style={{
        flexDirection: direction === "vertical" ? "column" : "row",
        gap: space[gap],
        alignItems: align ? ALIGN[align] : undefined,
        padding: padding ? space[padding] : undefined,
      }}
    >
      {children}
    </View>
  );
}

import type { ReactNode } from "react";
import { View } from "react-native";
import { color, space } from "./theme";

// The MOBILE mirror of web/src/ui — same names + closed props, RN primitives. Built by the
// consumer's Expo/Metro toolchain (which provides react-native), NOT by the framework's web-only
// `npm run check` — exactly like the old single-file ui.tsx it replaces.
export function Screen({ children }: { children: ReactNode }) {
  return <View style={{ flex: 1, backgroundColor: color.bg, padding: space.lg }}>{children}</View>;
}

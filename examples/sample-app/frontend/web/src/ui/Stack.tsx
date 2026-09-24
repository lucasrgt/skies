import type { ReactNode } from "react";
import { space, type Space } from "./theme";

const ALIGN = { start: "flex-start", center: "center", end: "flex-end", stretch: "stretch" } as const;

// Layout rhythm lives HERE: gap comes from the spacing scale, children never carry margins
// (vertical rhythm belongs to the container).
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
    <div
      style={{
        display: "flex",
        flexDirection: direction === "vertical" ? "column" : "row",
        gap: space[gap],
        alignItems: align ? ALIGN[align] : undefined,
        padding: padding ? space[padding] : undefined,
      }}
    >
      {children}
    </div>
  );
}

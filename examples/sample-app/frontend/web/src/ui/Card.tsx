import type { ReactNode } from "react";
import { color, radius, shadow, space, type Space } from "./theme";

// The raised surface.
export function Card({
  children,
  padding = "lg",
  listItem = false,
}: {
  children: ReactNode;
  padding?: Space;
  /** Maps the collection-row semantic to the platform accessibility tree. */
  listItem?: boolean;
}) {
  return (
    <div
      role={listItem ? "listitem" : undefined}
      style={{
        backgroundColor: color.surface,
        borderWidth: 1,
        borderStyle: "solid",
        borderColor: color.border,
        borderRadius: radius.lg,
        boxShadow: shadow.raised,
        padding: space[padding],
      }}
    >
      {children}
    </div>
  );
}

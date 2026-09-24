import type { ReactNode } from "react";
import { color, space } from "./theme";

// Page container — owns the page background and the readable content column, so screens never
// hand-roll page margins. The closed API starts here: no className,
// no style — a screen that needs different paint needs a different primitive, in ui/.
export function Screen({ children }: { children: ReactNode }) {
  return (
    <div style={{ minHeight: "100%", backgroundColor: color.bg, padding: space.lg }}>
      <div style={{ maxWidth: 720, margin: "0 auto" }}>{children}</div>
    </div>
  );
}

// The app's own UI components (web). Screens compose these instead of styling host elements; props are small unions
// (no className/style passthrough), so a missing primitive is added here rather than inlined. Events keep the DOM's
// names and shapes (`onClick`, `onChange` with the change event), so a react-hook-form `field.onChange` binds as is.
export { Screen } from "./Screen";
export { Stack } from "./Stack";
export { Text } from "./Text";
export { Button } from "./Button";
export { Field } from "./Field";
export { Input } from "./Input";
export { Card } from "./Card";
export { EmptyState, ErrorState } from "./states";

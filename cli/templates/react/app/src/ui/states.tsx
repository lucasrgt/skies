import { Button } from "./Button";
import { Stack } from "./Stack";
import { Text } from "./Text";

// The async sad/empty paths <Resource> renders — kit-composed (the kit dogfoods its own
// vocabulary), so every screen's empty and error read the same.
export function EmptyState({ title, description }: { title: string; description?: string }) {
  return (
    <Stack gap="xs" align="center" padding="xl">
      <Text role="heading">{title}</Text>
      {description ? <Text tone="muted">{description}</Text> : null}
    </Stack>
  );
}

export function ErrorState({
  title,
  retryLabel,
  onRetry,
}: {
  title: string;
  retryLabel?: string;
  onRetry?: () => void;
}) {
  return (
    <Stack gap="sm" align="center" padding="xl">
      <Text role="heading" tone="danger">
        {title}
      </Text>
      {onRetry && retryLabel ? <Button label={retryLabel} onClick={onRetry} variant="secondary" /> : null}
    </Stack>
  );
}

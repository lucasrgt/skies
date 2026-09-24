import { useTranslation } from "react-i18next";
import { Resource } from "@skiesjs/react";
// The app's component kit — the View reaches markup and styling through these names only.
import { Screen, Stack, Text, EmptyState } from "@/ui";
import { use{{ plural }}Model } from "./{{ plural }}.viewModel";
import type { {{ entity }} } from "./{{ plural }}.viewModel";

// VIEW — render only (SKYFE001). Consumes the resource through <Resource>, so loading / error / empty are handled by
// construction and the body only ever runs with resolved data. No isPending/isError here (SKYFE010).
export function {{ plural }}View() {
  const { t } = useTranslation("{{ lower }}");
  const { state } = use{{ plural }}Model();

  return (
    <Resource
      state={state.{{ collection }}}
      empty={
        <Screen>
          <EmptyState title={t("empty.title")} description={t("empty.description")} />
        </Screen>
      }
    >
      {({{ collection }}) => <{{ plural }}List {{ collection }}={{ '{' }}{{ collection }}} />}
    </Resource>
  );
}

function {{ plural }}List({ {{ collection }} }: { {{ collection }}: {{ entity }}[] }) {
  return (
    <Screen>
      <Stack>
        {{ '{' }}{{ collection }}.map((item) => (
          <Text key={item.id}>{item.name}</Text>
        ))}
      </Stack>
    </Screen>
  );
}

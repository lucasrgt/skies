import { useTranslation } from "react-i18next";
import { Resource } from "@skiesjs/react";
import { Screen, Stack, Text, EmptyState } from "@/ui";
import { use{{ plural }}Model } from "./{{ plural }}.viewModel";
import type { {{ entity }} } from "./{{ plural }}.viewModel";

// <Resource> renders loading, error, and empty, so the body only ever runs with resolved data.
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
{%- if key %}
        {{ '{' }}{{ collection }}.map((item) => (
          <Text key={item.{{ key }}}>{item.{{ display }}}</Text>
        ))}
{%- else %}
        {{ '{' }}{{ collection }}.map((item, index) => (
          <Text key={index}>{item.{{ display }}}</Text>
        ))}
{%- endif %}
      </Stack>
    </Screen>
  );
}

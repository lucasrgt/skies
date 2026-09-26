import { useTranslation } from "react-i18next";
import { Resource } from "@skiesjs/react";
import { Card, EmptyState, ErrorState, Screen, Stack, Text } from "@/ui";
import { useItemsModel } from "./Items.viewModel";
import type { Item } from "./Items.viewModel";

// CANONICAL LIST VIEW — every async branch renders through <Resource> (loading text, ErrorState with the spine's
// retry, EmptyState); the ready body is title + Card rows. Render only (SKYFE001); no isPending/isError here
// (SKYFE010).
export function ItemsView() {
  const { t } = useTranslation("items");
  const { state } = useItemsModel();

  return (
    <Resource
      state={state.items}
      loading={
        <Screen>
          <Stack gap="sm" align="center" padding="xl">
            <Text tone="muted">{t("loading")}</Text>
          </Stack>
        </Screen>
      }
      error={(message, retry) => (
        <Screen>
          <ErrorState title={message} retryLabel={t("retry")} onRetry={retry} />
        </Screen>
      )}
      empty={
        <Screen>
          <EmptyState title={t("empty.title")} description={t("empty.description")} />
        </Screen>
      }
    >
      {(items) => <ItemList items={items} />}
    </Resource>
  );
}

function ItemList({ items }: { items: Item[] }) {
  const { t } = useTranslation("items");
  return (
    <Screen>
      <Stack gap="lg">
        <Text role="title">{t("title")}</Text>
        <Stack gap="sm">
          {items.map((item) => (
            <Card key={item.id} padding="md" listItem>
              <Text>{item.name}</Text>
            </Card>
          ))}
        </Stack>
      </Stack>
    </Screen>
  );
}

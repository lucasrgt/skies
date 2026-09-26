import { useTranslation } from "react-i18next";
import { EmptyState, Screen } from "@/ui";

export function HomeView() {
  const { t } = useTranslation("home");

  return (
    <Screen>
      <EmptyState title={t("title")} description={t("description")} />
    </Screen>
  );
}

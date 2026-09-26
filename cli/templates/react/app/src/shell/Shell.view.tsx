import { useTranslation } from "react-i18next";
import { Link, Outlet } from "@tanstack/react-router";
import { Stack, Text } from "@/ui";

// The app frame every route renders inside: the title, the navigation, and the current route's View.
export function ShellView() {
  const { t } = useTranslation("shell");

  return (
    <Stack gap="none">
      <Stack direction="horizontal" gap="lg" align="center" padding="md">
        <Text role="heading">{t("title")}</Text>
        <Link to="/">{t("nav.home")}</Link>
      </Stack>
      <Outlet />
    </Stack>
  );
}

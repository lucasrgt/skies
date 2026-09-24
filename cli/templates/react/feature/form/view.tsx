import { useTranslation } from "react-i18next";
import { Controller } from "react-hook-form";
// The app's component kit — the View reaches markup and styling through these names only.
import { Button, Card, Field, Input, Screen, Stack, Text } from "@/ui";
import { use{{ name }}Model } from "./{{ name }}.viewModel";

// FORM VIEW — render only (SKYFE001): Screen > Stack > Text(title) > Card > one Field+Input per field > the
// role=alert command error > Button (loading while pending). Field errors render inside their Field (SKYFE032); the
// command's failure renders above the submit (SKYFE013).
export function {{ name }}View() {
  const { t } = useTranslation("{{ lower }}");
  const { control, submit, submitting, submitError, completed } = use{{ name }}Model();

  // The success surface. A routed app returns a declarative <Navigate> here instead (SKYFE015).
  if (completed) {
    return (
      <Screen>
        <Stack gap="sm" align="center" padding="xl">
          <Text role="heading">{t("done.title")}</Text>
          <Text tone="muted">{t("done.description")}</Text>
        </Stack>
      </Screen>
    );
  }

  return (
    <Screen>
      <Stack gap="lg">
        <Text role="title">{t("title")}</Text>
        <Card>
          <Stack gap="md">
            <Controller
              control={control}
              name="id"
              render={({ field, fieldState }) => (
                <Field
                  fieldId="id"
                  label={t("fields.id.label")}
                  hint={t("fields.id.hint")}
                  error={fieldState.error?.message}
                >
                  <Input id="id" value={field.value} onChangeText={field.onChange} />
                </Field>
              )}
            />
            {submitError ? (
              <Text role="label" tone="danger" alert>
                {submitError}
              </Text>
            ) : null}
            <Button label={t("submit")} onPress={submit} loading={submitting} />
          </Stack>
        </Card>
      </Stack>
    </Screen>
  );
}

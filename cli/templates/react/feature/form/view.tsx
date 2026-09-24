import { useTranslation } from "react-i18next";
import { Controller } from "react-hook-form";
import { Button, Card, Field, Input, Screen, Stack, Text } from "@/ui";
import { use{{ name }}Model } from "./{{ name }}.viewModel";
{%- if targets %}
import type { {{ name }}Target } from "./{{ name }}.viewModel";
{%- endif %}
{% if targets %}
// The route renders this with the record it acts on (its id from the path, its version from the loaded row).
export function {{ name }}View({ target }: { target: {{ name }}Target }) {
{%- else %}
export function {{ name }}View() {
{%- endif %}
  const { t } = useTranslation("{{ lower }}");
  const { control, submit, submitting, submitError, completed } = use{{ name }}Model({% if targets %}target{% endif %});

  // A routed app returns a declarative <Navigate> here instead.
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
{%- for field in fields %}
            <Controller
              control={control}
              name="{{ field.name }}"
              render={({ field, fieldState }) => (
                <Field fieldId="{{ field.name }}" label={t("fields.{{ field.name }}.label")} error={fieldState.error?.message}>
                  <Input
                    id="{{ field.name }}"
                    name={field.name}
                    value={field.value}
                    onChange={field.onChange}
                    onBlur={field.onBlur}
{%- if field.input == "number" %}
                    kind="number"
{%- endif %}
                  />
                </Field>
              )}
            />
{%- endfor %}
            {submitError ? (
              <Text role="label" tone="danger" alert>
                {submitError}
              </Text>
            ) : null}
            <Button label={t("submit")} onClick={submit} loading={submitting} />
          </Stack>
        </Card>
      </Stack>
    </Screen>
  );
}

import { useTranslation } from "react-i18next";
import { Controller } from "react-hook-form";
import { Button, Card, Field, Input, Screen, Stack, Text } from "@/ui";
import { useDepositModel } from "./Deposit.viewModel";

// CANONICAL FORM VIEW — Screen > Stack > Text(title) > Card >
// one Field+Input per field > the role=alert command error > Button(primary, loading while pending). Field-level
// errors render inside their Field (anatomy: label → control → hint|error); the command's failure renders above
// the submit (SKYFE013 made visible). Instantiate this shape for any create/edit screen — never compose from blank.
export function DepositView() {
  const { t } = useTranslation("deposit");
  const { control, submit, submitting, submitError, completed } = useDepositModel();

  // The success surface. A routed app returns a declarative <Redirect> here (SKYFE015); the sample renders done.
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
              name="walletId"
              render={({ field, fieldState }) => (
                <Field
                  fieldId="walletId"
                  label={t("fields.walletId.label")}
                  hint={t("fields.walletId.hint")}
                  error={fieldState.error?.message}
                >
                  <Input
                    id="walletId"
                    value={field.value}
                    onChangeText={field.onChange}
                    placeholder={t("fields.walletId.placeholder")}
                  />
                </Field>
              )}
            />
            <Controller
              control={control}
              name="amount"
              render={({ field, fieldState }) => (
                <Field fieldId="amount" label={t("fields.amount.label")} error={fieldState.error?.message}>
                  <Input id="amount" value={field.value} onChangeText={field.onChange} kind="number" />
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

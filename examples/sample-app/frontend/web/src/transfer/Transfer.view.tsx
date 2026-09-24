import { useTranslation } from "react-i18next";
import { Controller, type Control } from "react-hook-form";
import { Button, Card, Field, Input, Screen, Stack, Text } from "@/ui";
import { useTransferModel, type TransferForm } from "./Transfer.viewModel";

// The Deposit form's shape: Screen > Stack > Text(title) > Card > one Field+Input per field > the role=alert command
// error > Button(primary, loading while pending). Field errors render inside their Field; the command's failure
// renders above the submit.
export function TransferView() {
  const { t } = useTranslation("transfer");
  const { control, submit, submitting, submitError, completed } = useTransferModel();

  // The success surface. A routed app returns a declarative <Navigate> here (SKYFE015); the sample renders done.
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
            <WalletField control={control} name="fromWalletId" label={t("fields.fromWalletId.label")} />
            <WalletField control={control} name="toWalletId" label={t("fields.toWalletId.label")} />
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

function WalletField({
  control,
  name,
  label,
}: {
  control: Control<TransferForm>;
  name: "fromWalletId" | "toWalletId";
  label: string;
}) {
  const { t } = useTranslation("transfer");
  return (
    <Controller
      control={control}
      name={name}
      render={({ field, fieldState }) => (
        <Field fieldId={name} label={label} error={fieldState.error?.message}>
          <Input
            id={name}
            value={field.value}
            onChangeText={field.onChange}
            placeholder={t("fields.walletId.placeholder")}
          />
        </Field>
      )}
    />
  );
}

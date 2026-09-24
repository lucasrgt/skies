import { toAsyncState, type AsyncState } from "@skiesjs/react";
{%- if row %}
import type { {{ row }} } from "@/client.gen/model";
{%- endif %}
import { use{{ slice }} } from "@/client.gen/{{ client }}";
import i18n from "@/i18n";
{% if row %}
// The row the View renders: the contract's own type, so a changed contract breaks the build here.
export type {{ entity }} = {{ row }};
{%- else %}
// The row shape the View renders. Replace it with the generated contract type once the client exists.
export interface {{ entity }} {
  id: string;
  name: string;
}
{%- endif %}

export interface {{ plural }}Model {
  state: { {{ collection }}: AsyncState<{{ entity }}[]> };
}

export function use{{ plural }}Model(): {{ plural }}Model {
  const query = use{{ slice }}();

  const {{ collection }} = toAsyncState<{{ entity }}[]>(
    {
      isPending: query.isPending,
      isError: query.isError,
      data: query.data?.{{ collection }}?.items,
      refetch: query.refetch,
    },
    { errorMessage: i18n.t("{{ lower }}:error"), isEmpty: (list) => list.length === 0 },
  );

  return { state: { {{ collection }} } };
}

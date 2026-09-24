import { toAsyncState, type AsyncState } from "@skiesjs/react";
import { useList{{ plural }} } from "@/client.gen/{{ client }}";
import i18n from "@/i18n";

// The row shape the View renders. Replace it with the generated contract type once the client exists.
export interface {{ entity }} {
  id: string;
  name: string;
}

export interface {{ plural }}Model {
  state: { {{ collection }}: AsyncState<{{ entity }}[]> };
}

export function use{{ plural }}Model(): {{ plural }}Model {
  const query = useList{{ plural }}();

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

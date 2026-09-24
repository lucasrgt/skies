import { toAsyncState, type AsyncState } from "@skiesjs/react";
// The orval-generated read hook of the `List{{ plural }}` slice (`.WithName(nameof(List{{ plural }}))`) — the ONLY data
// the door touches.
import { useList{{ plural }} } from "@/client.gen/{{ client }}";
import i18n from "@/i18n";

// FEATURE UNIT — the ViewModel (the "data door", the front-side of a backend [Slice]). Only place that touches the
// generated client (SKYFE002), renders nothing so a spec case drives it with renderHook, exposes its resource as
// AsyncState<T> (the spine) so the View handles every state by construction.

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
      data: query.data?.{{ collection }},
      refetch: query.refetch,
    },
    { errorMessage: i18n.t("{{ lower }}:error"), isEmpty: (list) => list.length === 0 },
  );

  return { state: { {{ collection }} } };
}

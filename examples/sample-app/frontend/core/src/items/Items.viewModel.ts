import { toAsyncState, type AsyncState } from "@skiesjs/react";
// Illustrative: the orval-generated typed hook for the `list_items` slice — the ONLY data the door touches.
import { useListItems } from "@/client.gen/sample";
import i18n from "@/i18n";

// CANONICAL FEATURE UNIT — the ViewModel (the "data door", the front-side of a backend [Slice]). It is the only
// place that touches the generated client (SKYFE002), it is platform-agnostic so it tests in jsdom (SKYFE009), and
// it exposes its resource as an AsyncState<T> (the spine) so the View handles every state by construction.

export interface Item {
  id: string;
  name: string;
}

export interface ItemsModel {
  // One resource per screen, as AsyncState. (Commands — create/delete — would sit here as functions alongside.)
  state: { items: AsyncState<Item[]> };
}

export function useItemsModel(): ItemsModel {
  const query = useListItems();

  // Project the query into the spine. `errorMessage` is localized here (the spine never hardcodes copy);
  // `isEmpty` opts an empty list into the dedicated empty state.
  const items = toAsyncState<Item[]>(
    {
      isPending: query.isPending,
      isError: query.isError,
      data: query.data?.items,
      refetch: query.refetch,
    },
    { errorMessage: i18n.t("items:error"), isEmpty: (list) => list.length === 0 },
  );

  return { state: { items } };
}

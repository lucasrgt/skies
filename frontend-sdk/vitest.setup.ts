import { afterAll, afterEach, beforeAll, beforeEach } from "vitest";
import { delay, http, HttpResponse } from "msw";
import { setupServer } from "msw/node";
import { SAMPLE_API_BASE } from "../examples/sample-app/frontend/web/src/api";

// The sample's tests run the real generated client against an MSW stand-in for the backend.
const MISSING_WALLET = "99999999-9999-4999-8999-999999999999";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const server = setupServer();

beforeAll(() => server.listen({ onUnhandledRequest: "error" }));

beforeEach(() => {
  server.use(
    http.get(`${SAMPLE_API_BASE}/items`, () => HttpResponse.json({ items: [] })),
    http.post(`${SAMPLE_API_BASE}/deposit`, async ({ request }) => {
      await delay(50);
      const input = (await request.json()) as { walletId: string; amount: number };
      if (input.walletId === MISSING_WALLET) return new HttpResponse(null, { status: 404 });
      return HttpResponse.json({ walletId: input.walletId, balance: input.amount });
    }),
    // Every source wallet holds 100 here: a larger transfer is refused as the slice refuses an overdraw (422).
    http.post(`${SAMPLE_API_BASE}/wallets/transfer`, async ({ request }) => {
      await delay(50);
      const input = (await request.json()) as { fromWalletId: string; toWalletId: string; amount: number };
      if (input.amount > 100) {
        return HttpResponse.json({ code: "wallets.insufficient_funds" }, { status: 422 });
      }
      return HttpResponse.json({
        fromWalletId: input.fromWalletId,
        fromBalance: 100 - input.amount,
        toWalletId: input.toWalletId,
        toBalance: input.amount,
      });
    }),
  );
});

afterEach(() => server.resetHandlers());
afterAll(() => server.close());

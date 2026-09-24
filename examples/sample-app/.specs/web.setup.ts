import { afterAll, afterEach, beforeAll, beforeEach } from "vitest";
import { delay, http, HttpResponse } from "msw";
import { setupServer } from "msw/node";
import { SAMPLE_API_BASE } from "../frontend/web/src/api";

// The sample's web specs run the real client against this MSW stand-in for the backend. It belongs to the specs, not
// to the framework or to src/ (SKYFE003 keeps MSW out of production code): a feature that needs a new route adds its
// handler here. Every route matches the backend's real one (backend/Sample.Api, the /wallets group), so a client
// that calls the wrong path fails here as it would against the API.
const MISSING_WALLET = "99999999-9999-4999-8999-999999999999";

const server = setupServer();

beforeAll(() => server.listen({ onUnhandledRequest: "error" }));

beforeEach(() => {
  server.use(
    // The Items screen's list stands in for a list slice the sample backend does not have.
    http.get(`${SAMPLE_API_BASE}/items`, () => HttpResponse.json({ items: [] })),
    // Deposit: POST /wallets/deposit. An unknown wallet is the slice's 404 with its registry code.
    http.post(`${SAMPLE_API_BASE}/wallets/deposit`, async ({ request }) => {
      await delay(50);
      const input = (await request.json()) as { walletId: string; amount: number };
      if (input.walletId === MISSING_WALLET) {
        return HttpResponse.json({ code: "wallets.not_found" }, { status: 404 });
      }
      return HttpResponse.json({ walletId: input.walletId, balance: input.amount });
    }),
    // Transfer: POST /wallets/transfer. Every source wallet holds 100 here: a larger transfer is refused as the slice
    // refuses an overdraw (422).
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

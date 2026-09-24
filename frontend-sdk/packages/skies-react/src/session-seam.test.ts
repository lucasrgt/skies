import { describe, expect, it, vi } from "vitest";
import { createSessionSeam } from "./session-seam";

// The seam exists so a token write and the session-cache reset are ONE move — the scattered write that
// forgets the reset (and bounces a just-authenticated user back to login) must be unrepresentable.
describe("createSessionSeam", () => {
  it("signIn pairs the token write with the identity cache reset by construction", async () => {
    const setAccessToken = vi.fn();
    const onIdentityChanged = vi.fn();
    const seam = createSessionSeam({ setAccessToken, onIdentityChanged, refresh: async () => null });

    await seam.signIn({ accessToken: "jwt" });

    expect(setAccessToken).toHaveBeenCalledWith("jwt");
    expect(onIdentityChanged).toHaveBeenCalledOnce();
  });

  it("bootstrap re-mints the bearer from the refresh cookie and hands it to the sink", async () => {
    const setAccessToken = vi.fn();
    const refresh = vi.fn(async () => ({ accessToken: "jwt" }));
    const seam = createSessionSeam({ setAccessToken, onIdentityChanged: vi.fn(), refresh });

    const restored = await seam.bootstrapSession();

    expect(restored).toBe(true);
    expect(refresh).toHaveBeenCalledOnce();
    expect(setAccessToken).toHaveBeenCalledWith("jwt");
  });

  it("a failed bootstrap reports anonymous instead of throwing — no session is a state, not an error", async () => {
    const seam = createSessionSeam({
      setAccessToken: vi.fn(),
      onIdentityChanged: vi.fn(),
      refresh: async () => {
        throw new Error("401");
      },
    });

    expect(await seam.bootstrapSession()).toBe(false);
  });

  it("clearSession drops the token and resets the cache in one move", async () => {
    const setAccessToken = vi.fn();
    const onIdentityChanged = vi.fn();
    const seam = createSessionSeam({ setAccessToken, onIdentityChanged, refresh: async () => null });

    await seam.clearSession();

    expect(setAccessToken).toHaveBeenCalledWith(null);
    expect(onIdentityChanged).toHaveBeenCalledOnce();
  });

  // The split that kills the hostpoint cache-leak: a sign-in is an IDENTITY change (total wipe), a bootstrap is a
  // ROTATION of the same identity (light reset). Conflating them leaked user A's cache into user B's session.
  it("signIn fires the IDENTITY reset (total wipe), not the rotation reset", async () => {
    const onSessionChanged = vi.fn();
    const onIdentityChanged = vi.fn();
    const seam = createSessionSeam({
      setAccessToken: vi.fn(),
      refresh: async () => null,
      onSessionChanged,
      onIdentityChanged,
    });

    await seam.signIn({ accessToken: "jwt" });

    expect(onIdentityChanged).toHaveBeenCalledOnce();
    expect(onSessionChanged).not.toHaveBeenCalled();
  });

  it("bootstrap (rotation) fires only the LIGHT reset — never the identity wipe", async () => {
    const onSessionChanged = vi.fn();
    const onIdentityChanged = vi.fn();
    const seam = createSessionSeam({
      setAccessToken: vi.fn(),
      refresh: async () => ({ accessToken: "jwt" }),
      onSessionChanged,
      onIdentityChanged,
    });

    await seam.bootstrapSession();

    expect(onSessionChanged).toHaveBeenCalledOnce();
    expect(onIdentityChanged).not.toHaveBeenCalled();
  });

  it("clearSession (sign-out) fires the IDENTITY reset — the next user starts clean", async () => {
    const onSessionChanged = vi.fn();
    const onIdentityChanged = vi.fn();
    const seam = createSessionSeam({
      setAccessToken: vi.fn(),
      refresh: async () => null,
      onSessionChanged,
      onIdentityChanged,
    });

    await seam.clearSession();

    expect(onIdentityChanged).toHaveBeenCalledOnce();
    expect(onSessionChanged).not.toHaveBeenCalled();
  });

  // A cold start that double-invokes the boot effect (StrictMode), or a bootstrap racing the client's 401
  // interceptor, must not fire TWO refresh rotations — the backend's theft detection burns the family on the
  // replayed token (SKYFE029 at boot). bootstrapSession is single-flighted.
  it("concurrent bootstrapSession calls share ONE refresh rotation (no double-rotation at boot)", async () => {
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const refresh = vi.fn(() => gate.then(() => ({ accessToken: "jwt" })));
    const seam = createSessionSeam({ setAccessToken: vi.fn(), onIdentityChanged: vi.fn(), refresh });

    const a = seam.bootstrapSession();
    const b = seam.bootstrapSession();
    release();

    expect(await Promise.all([a, b])).toEqual([true, true]);
    expect(refresh).toHaveBeenCalledOnce();
  });
  it.each(["logout", "sign-in"])("ignores an old refresh after %s", async (change) => {
    let release!: (tokens: { accessToken: string }) => void;
    let token: string | null = "A";
    const onSessionChanged = vi.fn();
    const seam = createSessionSeam({
      setAccessToken: (value) => { token = value; },
      refresh: () => new Promise((resolve) => { release = resolve; }),
      onIdentityChanged: vi.fn(), onSessionChanged,
    });
    const pending = seam.bootstrapSession();
    if (change === "logout") await seam.clearSession();
    else await seam.signIn({ accessToken: "B" });
    release({ accessToken: "old-A" });
    expect(await pending).toBe(false);
    expect(token).toBe(change === "logout" ? null : "B");
    expect(onSessionChanged).not.toHaveBeenCalled();
  });

  it("a new identity refreshes without joining the previous identity's request", async () => {
    let release!: (tokens: { accessToken: string }) => void;
    const refresh = vi.fn()
      .mockImplementationOnce(() => new Promise((resolve) => { release = resolve; }))
      .mockResolvedValue({ accessToken: "fresh-B" });
    const setAccessToken = vi.fn();
    const seam = createSessionSeam({ refresh, setAccessToken, onIdentityChanged: vi.fn() });
    const old = seam.bootstrapSession();
    await seam.signIn({ accessToken: "B" });
    expect(await seam.bootstrapSession()).toBe(true);
    release({ accessToken: "old-A" });
    expect(await old).toBe(false);
    expect(setAccessToken).toHaveBeenLastCalledWith("fresh-B");
  });

  it("an empty refresh does not report a restored session", async () => {
    const onSessionChanged = vi.fn();
    const seam = createSessionSeam({ setAccessToken: vi.fn(), onIdentityChanged: vi.fn(),
      refresh: async () => null, onSessionChanged });
    expect(await seam.bootstrapSession()).toBe(false);
    expect(onSessionChanged).not.toHaveBeenCalled();
  });

});

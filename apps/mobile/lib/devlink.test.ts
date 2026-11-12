import { beforeEach, describe, expect, it, vi } from "vitest";

const { nativeModule, listeners, availability } = vi.hoisted(() => {
  const listeners = new Map<string, Set<(event: unknown) => void>>();
  const availability = { available: true };
  const nativeModule = {
    addListener: vi.fn((name: string, listener: (event: unknown) => void) => {
      const set = listeners.get(name) ?? new Set<(event: unknown) => void>();
      set.add(listener);
      listeners.set(name, set);
      return {
        remove() {
          set.delete(listener);
        },
      };
    }),
    connect: vi.fn(async () => undefined),
    disconnect: vi.fn(async () => undefined),
    requestSync: vi.fn(async () => undefined),
    applyCustomization: vi.fn(async (_patch: string) => undefined),
    currentState: vi.fn(async () => ({
      status: "idle",
      connected: false,
      revision: -1,
      customization: "",
    })),
  };
  return { nativeModule, listeners, availability };
});

vi.mock("expo-modules-core", () => ({
  requireOptionalNativeModule: () => (availability.available ? nativeModule : null),
}));

vi.mock("react-native", () => ({ Platform: { OS: "android" } }));

const customization = {
  appearance: {
    face: "hybrid",
    palette: "cyan_violet",
    show_date: true,
    show_seconds: false,
    show_complications: true,
    complications: [
      { kind: "heart_rate", accent: "coral", visible: true, style: "icon_value" },
      { kind: "steps", accent: "cyan", visible: true, style: "icon_value" },
      { kind: "battery", accent: "mint", visible: true, style: "icon_value" },
    ],
    glow: 60,
  },
  use_24h: true,
  reduce_motion: false,
};

async function load() {
  return import("./devlink");
}

describe("mobile watchos development link", () => {
  beforeEach(() => {
    listeners.clear();
    availability.available = true;
    nativeModule.connect.mockClear();
    nativeModule.disconnect.mockClear();
    nativeModule.requestSync.mockClear();
    nativeModule.applyCustomization.mockClear();
    nativeModule.currentState.mockClear();
    vi.resetModules();
  });

  it("is available only when the native module is present", async () => {
    const { devLinkAvailable } = await load();
    expect(devLinkAvailable()).toBe(true);
    availability.available = false;
    vi.resetModules();
    const unavailable = await load();
    expect(unavailable.devLinkAvailable()).toBe(false);
  });

  it("passes the host, the port and the token to the native module", async () => {
    const { devLink } = await load();
    await devLink.connect({ port: 4599, token: "session-value" });
    expect(nativeModule.connect).toHaveBeenCalledWith("127.0.0.1", 4599, "session-value");
    await devLink.connect({ host: "10.0.2.2", port: 4600, token: "session-value" });
    expect(nativeModule.connect).toHaveBeenLastCalledWith("10.0.2.2", 4600, "session-value");
  });

  it("refuses to work without the native module", async () => {
    availability.available = false;
    vi.resetModules();
    const { devLink } = await load();
    await expect(devLink.connect({ port: 4599, token: "session-value" })).rejects.toThrow(
      /Android lab build/,
    );
  });

  it("sends a change as json", async () => {
    const { devLink } = await load();
    await devLink.applyCustomization({ appearance: { face: "classic" } });
    expect(nativeModule.applyCustomization).toHaveBeenCalledTimes(1);
    const sent = nativeModule.applyCustomization.mock.calls[0]?.[0];
    expect(typeof sent).toBe("string");
    expect(JSON.parse(sent as string)).toEqual({ appearance: { face: "classic" } });
  });

  it("reads a customization the simulator reported", async () => {
    const { parseCustomization } = await load();
    const parsed = parseCustomization(JSON.stringify(customization));
    expect(parsed).not.toBeNull();
    expect(parsed?.appearance.face).toBe("hybrid");
    expect(parsed?.appearance.complications).toHaveLength(3);
    expect(parsed?.use_24h).toBe(true);
  });

  it("refuses a customization that does not match the protocol", async () => {
    const { parseCustomization } = await load();
    expect(parseCustomization(null)).toBeNull();
    expect(parseCustomization("")).toBeNull();
    expect(parseCustomization("not json")).toBeNull();
    expect(parseCustomization(JSON.stringify({ ...customization, use_24h: "yes" }))).toBeNull();
    expect(parseCustomization(JSON.stringify({ ...customization, appearance: {} }))).toBeNull();
    expect(
      parseCustomization(
        JSON.stringify({
          ...customization,
          appearance: { ...customization.appearance, face: "unknown_face" },
        }),
      ),
    ).toBeNull();
    expect(
      parseCustomization(
        JSON.stringify({
          ...customization,
          appearance: { ...customization.appearance, glow: 200 },
        }),
      ),
    ).toBeNull();
    expect(
      parseCustomization(
        JSON.stringify({
          ...customization,
          appearance: {
            ...customization.appearance,
            complications: customization.appearance.complications.slice(0, 2),
          },
        }),
      ),
    ).toBeNull();
  });

  it("reports the current state of the link", async () => {
    nativeModule.currentState.mockResolvedValueOnce({
      status: "connected",
      connected: true,
      revision: 7,
      customization: JSON.stringify(customization),
    });
    const { devLink } = await load();
    const state = await devLink.currentState();
    expect(state.status).toBe("connected");
    expect(state.connected).toBe(true);
    expect(state.revision).toBe(7);
    expect(state.customization?.appearance.palette).toBe("cyan_violet");
  });

  it("reports no customization when the state is unknown", async () => {
    nativeModule.currentState.mockResolvedValueOnce({
      status: "idle",
      connected: false,
      revision: -1,
      customization: "",
    });
    const { devLink } = await load();
    const state = await devLink.currentState();
    expect(state.customization).toBeNull();
    expect(state.revision).toBe(-1);
  });

  it("passes the simulator state on and drops a broken one", async () => {
    const { watchDevLinkCustomization } = await load();
    const seen: number[] = [];
    const subscription = watchDevLinkCustomization((event) => seen.push(event.revision));
    const emit = listeners.get("onDevLinkCustomization");
    emit?.forEach((listener) =>
      listener({ revision: 3, customization: JSON.stringify(customization) }),
    );
    emit?.forEach((listener) => listener({ revision: 4, customization: "{broken" }));
    expect(seen).toEqual([3]);
    subscription.remove();
  });

  it("keeps the link state, the acknowledgements and the errors apart", async () => {
    const { watchDevLinkState, watchDevLinkAck, watchDevLinkError } = await load();
    const states: string[] = [];
    const acks: unknown[] = [];
    const errors: unknown[] = [];
    watchDevLinkState((event) => states.push(event.status));
    watchDevLinkAck((event) => acks.push(event));
    watchDevLinkError((event) => errors.push(event));
    listeners.get("onDevLinkState")?.forEach((listener) => listener({ status: "connected" }));
    listeners.get("onDevLinkAck")?.forEach((listener) =>
      listener({ requestId: 2, ok: true, revision: 3 }),
    );
    listeners.get("onDevLinkError")?.forEach((listener) =>
      listener({ code: "conflict", message: "stale", requestId: 2 }),
    );
    expect(states).toEqual(["connected"]);
    expect(acks).toEqual([{ requestId: 2, ok: true, revision: 3 }]);
    expect(errors).toEqual([{ code: "conflict", message: "stale", requestId: 2 }]);
  });
});

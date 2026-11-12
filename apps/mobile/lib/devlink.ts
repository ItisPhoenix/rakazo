import { requireOptionalNativeModule, type NativeModule } from "expo-modules-core";
import { Platform } from "react-native";

/**
 * The Charlie WatchOS development link, Android side.
 *
 * The client talks to the Rust simulator over a loopback TCP socket that an
 * `adb reverse` tunnel forwards to the Windows host. It carries watch
 * customization only.
 *
 * This link is a development tool. It is not Charlie's assistant backend and
 * it has nothing to do with the physical watch: connecting to a simulator
 * never means Charlie is online and never means a watch is paired.
 *
 * The session token is supplied by the caller at connection time. It is never
 * stored here and never logged.
 */

export type DevLinkStatus =
  | "idle"
  | "connecting"
  | "authenticating"
  | "connected"
  | "disconnected"
  | "error";

export type DevLinkFace = "hybrid" | "minimal" | "classic" | "dashboard";

export type DevLinkPalette =
  | "cyan_violet"
  | "electric_blue"
  | "emerald"
  | "sunset"
  | "rose";

export type DevLinkComplicationKind =
  | "heart_rate"
  | "steps"
  | "battery"
  | "calories"
  | "distance"
  | "sleep"
  | "blood_oxygen"
  | "workout"
  | "running"
  | "walking"
  | "cycling"
  | "activity_goals"
  | "exercise_duration"
  | "bluetooth"
  | "notifications"
  | "weather"
  | "alarm"
  | "timer"
  | "stopwatch"
  | "music"
  | "settings"
  | "phone_finder"
  | "brightness"
  | "do_not_disturb"
  | "charlie";

export type DevLinkAccent = "theme" | "coral" | "cyan" | "mint" | "amber" | "violet";

export type DevLinkComplicationStyle = "icon_value" | "icon_only" | "value_only";

export type DevLinkComplicationSlot = {
  kind: DevLinkComplicationKind;
  accent: DevLinkAccent;
  visible: boolean;
  style: DevLinkComplicationStyle;
};

export type DevLinkAppearance = {
  face: DevLinkFace;
  palette: DevLinkPalette;
  show_date: boolean;
  show_seconds: boolean;
  show_complications: boolean;
  complications: [DevLinkComplicationSlot, DevLinkComplicationSlot, DevLinkComplicationSlot];
  glow: number;
};

export type DevLinkCustomization = {
  appearance: DevLinkAppearance;
  use_24h: boolean;
  reduce_motion: boolean;
};

/** A change applied on top of the customization the simulator reported. */
export type DevLinkCustomizationPatch = {
  appearance?: Partial<DevLinkAppearance>;
  use_24h?: boolean;
  reduce_motion?: boolean;
};

export type DevLinkStateEvent = { status: DevLinkStatus; reason?: string };

export type DevLinkCustomizationEvent = {
  revision: number;
  customization: DevLinkCustomization;
};

export type DevLinkAckEvent = {
  requestId: number;
  ok: boolean;
  revision?: number;
  code?: string;
};

export type DevLinkErrorEvent = { code: string; message: string; requestId?: number };

export type DevLinkSnapshot = {
  status: DevLinkStatus;
  connected: boolean;
  revision: number;
  customization: DevLinkCustomization | null;
};

type Subscription = { remove(): void };

type NativeDevLink = NativeModule & {
  addListener(
    eventName: "onDevLinkState",
    listener: (state: DevLinkStateEvent) => void,
  ): Subscription;
  addListener(
    eventName: "onDevLinkCustomization",
    listener: (event: { revision: number; customization: string }) => void,
  ): Subscription;
  addListener(eventName: "onDevLinkAck", listener: (event: DevLinkAckEvent) => void): Subscription;
  addListener(
    eventName: "onDevLinkError",
    listener: (event: DevLinkErrorEvent) => void,
  ): Subscription;
  connect(host: string, port: number, token: string): Promise<void>;
  disconnect(): Promise<void>;
  requestSync(): Promise<number>;
  applyCustomization(patchJson: string): Promise<number>;
  isLabBuild(): boolean;
  currentState(): Promise<{
    status: string;
    connected: boolean;
    revision: number;
    customization: string;
  }>;
};

const module =
  Platform.OS === "android" ? requireOptionalNativeModule<NativeDevLink>("RakazoDevLink") : null;

const FACES: readonly DevLinkFace[] = ["hybrid", "minimal", "classic", "dashboard"];
const PALETTES: readonly DevLinkPalette[] = [
  "cyan_violet",
  "electric_blue",
  "emerald",
  "sunset",
  "rose",
];
const KINDS: readonly DevLinkComplicationKind[] = [
  "heart_rate",
  "steps",
  "battery",
  "calories",
  "distance",
  "sleep",
  "blood_oxygen",
  "workout",
  "running",
  "walking",
  "cycling",
  "activity_goals",
  "exercise_duration",
  "bluetooth",
  "notifications",
  "weather",
  "alarm",
  "timer",
  "stopwatch",
  "music",
  "settings",
  "phone_finder",
  "brightness",
  "do_not_disturb",
  "charlie",
];
const ACCENTS: readonly DevLinkAccent[] = ["theme", "coral", "cyan", "mint", "amber", "violet"];
const STYLES: readonly DevLinkComplicationStyle[] = ["icon_value", "icon_only", "value_only"];

/** True when this build carries the development link. */
export function devLinkAvailable(): boolean {
  return module !== null;
}

/**
 * True when this build may talk to a development simulator.
 *
 * Only the Charlie Wrist Lab build may do so. The native module refuses
 * every call in any other build; this lets the user interface say so before
 * a button is pressed instead of after it fails.
 */
export function devLinkLabBuild(): boolean {
  if (!module) return false;
  try {
    return module.isLabBuild() === true;
  } catch {
    // A native module without the check is treated as a build that may not
    // talk to a simulator.
    return false;
  }
}

function oneOf<T extends string>(value: unknown, allowed: readonly T[]): T | null {
  return typeof value === "string" && (allowed as readonly string[]).includes(value)
    ? (value as T)
    : null;
}

function parseSlot(value: unknown): DevLinkComplicationSlot | null {
  if (!value || typeof value !== "object") return null;
  const slot = value as Record<string, unknown>;
  const kind = oneOf(slot.kind, KINDS);
  const accent = oneOf(slot.accent, ACCENTS);
  const style = oneOf(slot.style, STYLES);
  if (!kind || !accent || !style || typeof slot.visible !== "boolean") return null;
  return { kind, accent, visible: slot.visible, style };
}

function parseAppearance(value: unknown): DevLinkAppearance | null {
  if (!value || typeof value !== "object") return null;
  const appearance = value as Record<string, unknown>;
  const face = oneOf(appearance.face, FACES);
  const palette = oneOf(appearance.palette, PALETTES);
  if (!face || !palette) return null;
  if (typeof appearance.show_date !== "boolean") return null;
  if (typeof appearance.show_seconds !== "boolean") return null;
  if (typeof appearance.show_complications !== "boolean") return null;
  if (typeof appearance.glow !== "number" || appearance.glow < 0 || appearance.glow > 100) {
    return null;
  }
  const slots = appearance.complications;
  if (!Array.isArray(slots) || slots.length !== 3) return null;
  const parsed = slots.map(parseSlot);
  if (parsed.some((slot) => slot === null)) return null;
  return {
    face,
    palette,
    show_date: appearance.show_date,
    show_seconds: appearance.show_seconds,
    show_complications: appearance.show_complications,
    complications: parsed as [
      DevLinkComplicationSlot,
      DevLinkComplicationSlot,
      DevLinkComplicationSlot,
    ],
    glow: appearance.glow,
  };
}

/**
 * Read a customization the simulator reported.
 *
 * Anything that does not match the protocol is refused instead of being
 * passed on, so a broken simulator can never put a half known watch state into
 * the app.
 */
export function parseCustomization(text: string | null | undefined): DevLinkCustomization | null {
  if (!text) return null;
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    return null;
  }
  if (!parsed || typeof parsed !== "object") return null;
  const customization = parsed as Record<string, unknown>;
  const appearance = parseAppearance(customization.appearance);
  if (!appearance) return null;
  if (typeof customization.use_24h !== "boolean") return null;
  if (typeof customization.reduce_motion !== "boolean") return null;
  return {
    appearance,
    use_24h: customization.use_24h,
    reduce_motion: customization.reduce_motion,
  };
}

function requireModule(): NativeDevLink {
  if (!module) throw new Error("the WatchOS development link needs an Android lab build");
  return module;
}

export const devLink = {
  /**
   * Connect to the simulator.
   *
   * The link is only usable once the simulator answered the handshake, so a
   * resolved promise here means "the socket is open", not "the link works".
   */
  connect: async (options: { host?: string; port: number; token: string }) =>
    requireModule().connect(options.host ?? "127.0.0.1", options.port, options.token),

  /** Close the link and release the socket. */
  disconnect: async () => requireModule().disconnect(),

  /**
   * Ask the simulator for the committed customization.
   *
   * The returned id belongs to the request, so the answer can be told apart
   * from the answer to a change.
   */
  requestSync: async () => requireModule().requestSync(),

  /**
   * Change the customization.
   *
   * The change is applied on top of the customization the simulator last
   * reported, so parts that are not mentioned keep their value.
   *
   * The returned id belongs to the request, so the answer can be told apart
   * from the answer to a sync.
   */
  applyCustomization: async (patch: DevLinkCustomizationPatch) =>
    requireModule().applyCustomization(JSON.stringify(patch)),

  /** The state the link is in right now. */
  currentState: async (): Promise<DevLinkSnapshot> => {
    const state = await requireModule().currentState();
    return {
      status: state.status as DevLinkStatus,
      connected: state.connected,
      revision: state.revision,
      customization: parseCustomization(state.customization),
    };
  },
};

const noopSubscription = (): Subscription => ({ remove() {} });

export const watchDevLinkState = (listener: (event: DevLinkStateEvent) => void): Subscription =>
  module?.addListener("onDevLinkState", listener) ?? noopSubscription();

export const watchDevLinkCustomization = (
  listener: (event: DevLinkCustomizationEvent) => void,
): Subscription =>
  module?.addListener("onDevLinkCustomization", (event) => {
    const customization = parseCustomization(event.customization);
    if (!customization) return;
    listener({ revision: event.revision, customization });
  }) ?? noopSubscription();

export const watchDevLinkAck = (listener: (event: DevLinkAckEvent) => void): Subscription =>
  module?.addListener("onDevLinkAck", listener) ?? noopSubscription();

export const watchDevLinkError = (listener: (event: DevLinkErrorEvent) => void): Subscription =>
  module?.addListener("onDevLinkError", listener) ?? noopSubscription();

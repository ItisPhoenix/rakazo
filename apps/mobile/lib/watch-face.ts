import type {
  DevLinkAccent,
  DevLinkAppearance,
  DevLinkComplicationKind,
  DevLinkComplicationSlot,
  DevLinkComplicationStyle,
  DevLinkCustomization,
  DevLinkCustomizationPatch,
  DevLinkFace,
  DevLinkPalette,
} from "./devlink";

/**
 * The watch customization the Android screen edits.
 *
 * The screen never keeps a full copy of the watch state. It keeps the state
 * the simulator reported and, on top of it, only the values the person
 * actually changed. That is what makes a conflict recoverable: when the
 * simulator reports a newer state, the changes are applied on top of that
 * newer state instead of restoring an outdated copy of it.
 *
 * The values, the names and the rules here mirror the simulator's own watch
 * protocol. The protocol itself is not changed by this file.
 */

/** One choice the user can make, with the name the watch shows. */
export type Choice<T extends string> = { value: T; label: string };

/** The four watch faces the simulator offers. */
export const FACE_CHOICES: readonly Choice<DevLinkFace>[] = [
  { value: "hybrid", label: "Hybrid" },
  { value: "minimal", label: "Minimal" },
  { value: "classic", label: "Classic" },
  { value: "dashboard", label: "Dashboard" },
];

/** The five colour palettes the simulator offers. */
export const PALETTE_CHOICES: readonly Choice<DevLinkPalette>[] = [
  { value: "cyan_violet", label: "Cyan & Violet" },
  { value: "electric_blue", label: "Electric Blue" },
  { value: "emerald", label: "Emerald" },
  { value: "sunset", label: "Sunset" },
  { value: "rose", label: "Rose" },
];

/** The accents a complication slot can use. */
export const ACCENT_CHOICES: readonly Choice<DevLinkAccent>[] = [
  { value: "theme", label: "Theme" },
  { value: "coral", label: "Coral" },
  { value: "cyan", label: "Cyan" },
  { value: "mint", label: "Mint" },
  { value: "amber", label: "Amber" },
  { value: "violet", label: "Violet" },
];

/** The ways a complication slot can show its value. */
export const STYLE_CHOICES: readonly Choice<DevLinkComplicationStyle>[] = [
  { value: "icon_value", label: "Icon & value" },
  { value: "icon_only", label: "Icon only" },
  { value: "value_only", label: "Value only" },
];

/** The kinds a complication slot can show, named as the watch names them. */
export const KIND_CHOICES: readonly Choice<DevLinkComplicationKind>[] = [
  { value: "heart_rate", label: "Heart Rate" },
  { value: "steps", label: "Steps" },
  { value: "battery", label: "Battery" },
  { value: "calories", label: "Calories" },
  { value: "distance", label: "Distance" },
  { value: "sleep", label: "Sleep" },
  { value: "blood_oxygen", label: "Blood Oxygen" },
  { value: "workout", label: "Workout" },
  { value: "running", label: "Running" },
  { value: "walking", label: "Walking" },
  { value: "cycling", label: "Cycling" },
  { value: "activity_goals", label: "Activity Goals" },
  { value: "exercise_duration", label: "Exercise Duration" },
  { value: "bluetooth", label: "Bluetooth" },
  { value: "notifications", label: "Notifications" },
  { value: "weather", label: "Weather" },
  { value: "alarm", label: "Alarm" },
  { value: "timer", label: "Timer" },
  { value: "stopwatch", label: "Stopwatch" },
  { value: "music", label: "Music" },
  { value: "settings", label: "Settings" },
  { value: "phone_finder", label: "Phone Finder" },
  { value: "brightness", label: "Brightness" },
  { value: "do_not_disturb", label: "Do Not Disturb" },
  { value: "charlie", label: "Charlie" },
];

/** The values a person may change in one complication slot. */
export type SlotEdits = Partial<DevLinkComplicationSlot>;

/**
 * Everything the person changed on the screen.
 *
 * A missing value means "not changed", so the value the simulator reported is
 * used instead.
 */
export type WatchEdits = {
  face?: DevLinkFace;
  palette?: DevLinkPalette;
  show_date?: boolean;
  show_seconds?: boolean;
  show_complications?: boolean;
  glow?: number;
  use_24h?: boolean;
  reduce_motion?: boolean;
  /** The changes per complication slot. A slot without changes is `null`. */
  slots: (SlotEdits | null)[];
};

/** The number of complication slots the watch has. */
export const SLOT_COUNT = 3;

/** The slot the simulator starts with, used when nothing has been reported yet. */
const DEFAULT_SLOT: DevLinkComplicationSlot = {
  kind: "heart_rate",
  accent: "coral",
  visible: true,
  style: "icon_value",
};

/** A fresh set of changes: nothing has been changed yet. */
export function emptyEdits(): WatchEdits {
  return { slots: [null, null, null] };
}

/** Change one of the watch's own values. */
export function setEdit<
  K extends
    | "face"
    | "palette"
    | "show_date"
    | "show_seconds"
    | "show_complications"
    | "glow"
    | "use_24h"
    | "reduce_motion",
>(edits: WatchEdits, key: K, value: WatchEdits[K]): WatchEdits {
  return { ...edits, [key]: value };
}

/** Change one value of one complication slot. */
export function setSlotEdit<K extends keyof SlotEdits>(
  edits: WatchEdits,
  slot: number,
  key: K,
  value: SlotEdits[K],
): WatchEdits {
  if (slot < 0 || slot >= SLOT_COUNT) return edits;
  const slots = [...edits.slots];
  slots[slot] = { ...(slots[slot] ?? {}), [key]: value };
  return { ...edits, slots };
}

/** True when the person changed anything at all. */
export function hasEdits(edits: WatchEdits): boolean {
  if (edits.slots.some((slot) => slot !== null)) return true;
  return (
    edits.face !== undefined ||
    edits.palette !== undefined ||
    edits.show_date !== undefined ||
    edits.show_seconds !== undefined ||
    edits.show_complications !== undefined ||
    edits.glow !== undefined ||
    edits.use_24h !== undefined ||
    edits.reduce_motion !== undefined
  );
}

/**
 * The slot the screen shows at a position.
 *
 * The simulator always reports three slots; the fallback only covers the
 * moment before anything has been reported.
 */
function slotAt(customization: DevLinkCustomization, slot: number): DevLinkComplicationSlot {
  return customization.appearance.complications[slot] ?? DEFAULT_SLOT;
}

/** One slot with the changed values of that slot applied to it. */
function mergeSlot(
  base: DevLinkComplicationSlot,
  changed: SlotEdits | null,
): DevLinkComplicationSlot {
  if (!changed) return base;
  return {
    kind: changed.kind ?? base.kind,
    accent: changed.accent ?? base.accent,
    visible: changed.visible ?? base.visible,
    style: changed.style ?? base.style,
  };
}

/**
 * The value a slot shows on the screen.
 *
 * A changed value wins; otherwise the value the simulator reported is used.
 */
export function slotField<K extends keyof DevLinkComplicationSlot>(
  authoritative: DevLinkCustomization,
  edits: WatchEdits,
  slot: number,
  key: K,
): DevLinkComplicationSlot[K] {
  const changed = edits.slots[slot]?.[key];
  if (changed !== undefined) return changed;
  return slotAt(authoritative, slot)[key];
}

/**
 * The change to send to the simulator.
 *
 * Only the values the person changed are mentioned, so the simulator keeps
 * everything else. The complication slots are sent as a whole list when any
 * slot changed, because the simulator replaces the list instead of merging it
 * slot by slot.
 *
 * Returns `null` when nothing was changed.
 */
export function buildPatch(
  authoritative: DevLinkCustomization,
  edits: WatchEdits,
): DevLinkCustomizationPatch | null {
  const appearance: Partial<DevLinkAppearance> = {};
  if (edits.face !== undefined) appearance.face = edits.face;
  if (edits.palette !== undefined) appearance.palette = edits.palette;
  if (edits.show_date !== undefined) appearance.show_date = edits.show_date;
  if (edits.show_seconds !== undefined) appearance.show_seconds = edits.show_seconds;
  if (edits.show_complications !== undefined) {
    appearance.show_complications = edits.show_complications;
  }
  if (edits.glow !== undefined) appearance.glow = edits.glow;

  if (edits.slots.some((slot) => slot !== null)) {
    const base = authoritative.appearance.complications;
    appearance.complications = [
      mergeSlot(base[0] ?? DEFAULT_SLOT, edits.slots[0] ?? null),
      mergeSlot(base[1] ?? DEFAULT_SLOT, edits.slots[1] ?? null),
      mergeSlot(base[2] ?? DEFAULT_SLOT, edits.slots[2] ?? null),
    ];
  }

  const patch: DevLinkCustomizationPatch = {};
  if (Object.keys(appearance).length > 0) patch.appearance = appearance;
  if (edits.use_24h !== undefined) patch.use_24h = edits.use_24h;
  if (edits.reduce_motion !== undefined) patch.reduce_motion = edits.reduce_motion;
  return Object.keys(patch).length > 0 ? patch : null;
}

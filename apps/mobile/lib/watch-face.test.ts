import { describe, expect, it } from "vitest";
import type { DevLinkCustomization } from "./devlink";
import {
  ACCENT_CHOICES,
  buildPatch,
  emptyEdits,
  FACE_CHOICES,
  hasEdits,
  KIND_CHOICES,
  PALETTE_CHOICES,
  STYLE_CHOICES,
  setEdit,
  setSlotEdit,
  slotField,
} from "./watch-face";

const customization = (): DevLinkCustomization => ({
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
});

describe("watch customization choices", () => {
  it("offers the four faces the simulator has", () => {
    expect(FACE_CHOICES.map((choice) => choice.value)).toEqual([
      "hybrid",
      "minimal",
      "classic",
      "dashboard",
    ]);
  });

  it("offers the five palettes the simulator has", () => {
    expect(PALETTE_CHOICES.map((choice) => choice.value)).toEqual([
      "cyan_violet",
      "electric_blue",
      "emerald",
      "sunset",
      "rose",
    ]);
  });

  it("offers every complication kind the simulator has", () => {
    expect(KIND_CHOICES).toHaveLength(25);
    expect(new Set(KIND_CHOICES.map((choice) => choice.value)).size).toBe(25);
  });

  it("offers every accent and style the simulator has", () => {
    expect(ACCENT_CHOICES.map((choice) => choice.value)).toEqual([
      "theme",
      "coral",
      "cyan",
      "mint",
      "amber",
      "violet",
    ]);
    expect(STYLE_CHOICES.map((choice) => choice.value)).toEqual([
      "icon_value",
      "icon_only",
      "value_only",
    ]);
  });
});

describe("watch customization changes", () => {
  it("has no change to send before anything is changed", () => {
    const edits = emptyEdits();
    expect(hasEdits(edits)).toBe(false);
    expect(buildPatch(customization(), edits)).toBeNull();
  });

  it("sends only the value that was changed", () => {
    const edits = setEdit(emptyEdits(), "face", "classic");
    expect(hasEdits(edits)).toBe(true);
    expect(buildPatch(customization(), edits)).toEqual({
      appearance: { face: "classic" },
    });
  });

  it("keeps a false value that was turned off", () => {
    const edits = setEdit(setEdit(emptyEdits(), "show_seconds", true), "show_seconds", false);
    expect(buildPatch(customization(), edits)).toEqual({
      appearance: { show_seconds: false },
    });
  });

  it("keeps a glow of zero", () => {
    const edits = setEdit(emptyEdits(), "glow", 0);
    expect(buildPatch(customization(), edits)).toEqual({ appearance: { glow: 0 } });
  });

  it("sends the clock and motion values on their own", () => {
    const edits = setEdit(setEdit(emptyEdits(), "use_24h", false), "reduce_motion", true);
    expect(buildPatch(customization(), edits)).toEqual({
      use_24h: false,
      reduce_motion: true,
    });
  });

  it("sends every slot when one slot changed", () => {
    const edits = setSlotEdit(emptyEdits(), 1, "kind", "cycling");
    const patch = buildPatch(customization(), edits);
    expect(patch?.appearance?.complications).toEqual([
      { kind: "heart_rate", accent: "coral", visible: true, style: "icon_value" },
      { kind: "cycling", accent: "cyan", visible: true, style: "icon_value" },
      { kind: "battery", accent: "mint", visible: true, style: "icon_value" },
    ]);
  });

  it("sends the changed part of a slot and keeps the rest", () => {
    const edits = setSlotEdit(setSlotEdit(emptyEdits(), 0, "accent", "amber"), 0, "visible", false);
    expect(buildPatch(customization(), edits)?.appearance?.complications?.[0]).toEqual({
      kind: "heart_rate",
      accent: "amber",
      visible: false,
      style: "icon_value",
    });
  });

  it("ignores a slot that does not exist", () => {
    const edits = setSlotEdit(emptyEdits(), 3, "kind", "cycling");
    expect(hasEdits(edits)).toBe(false);
  });
});

describe("watch customization against a newer simulator state", () => {
  it("shows the changed value on the screen", () => {
    const authoritative = customization();
    const edits = setEdit(emptyEdits(), "face", "dashboard");
    expect(slotField(authoritative, edits, 0, "kind")).toBe("heart_rate");
  });

  it("keeps the changed value when the simulator reports a newer state", () => {
    const edits = setSlotEdit(emptyEdits(), 1, "kind", "cycling");

    // The simulator committed something else in the meantime.
    const newer = customization();
    newer.appearance.palette = "emerald";

    // The change is sent on top of the newer state, so the part the person
    // did not touch keeps the simulator's value.
    const patch = buildPatch(newer, edits);
    expect(patch?.appearance?.complications?.[1].kind).toBe("cycling");
    expect(patch?.appearance?.palette).toBeUndefined();
  });

  it("does not send a value the person did not change", () => {
    const newer = customization();
    newer.appearance.glow = 10;
    const edits = setEdit(emptyEdits(), "palette", "rose");
    const patch = buildPatch(newer, edits);
    expect(patch).toEqual({ appearance: { palette: "rose" } });
  });
});

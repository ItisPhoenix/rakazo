import { describe, expect, it } from "vitest";
import type { DevLinkCustomization } from "./devlink";
import { emptyEdits, setEdit, setSlotEdit } from "./watch-face";
import type { LinkEvent, LinkState } from "./watch-link-state";
import { initialLinkState, reduceLink } from "./watch-link-state";

const customization = (face: string = "hybrid"): DevLinkCustomization => ({
  appearance: {
    face: face as DevLinkCustomization["appearance"]["face"],
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

/** Fold a list of events into the state, one after the other. */
function fold(events: LinkEvent[], from: LinkState = initialLinkState()): LinkState {
  return events.reduce(reduceLink, from);
}

describe("watch link state", () => {
  it("starts without a state and without a notice", () => {
    const state = initialLinkState();
    expect(state.status).toBe("idle");
    expect(state.revision).toBe(-1);
    expect(state.authoritative).toBeNull();
    expect(state.edits).toEqual(emptyEdits());
    expect(state.notice).toBeNull();
  });

  it("takes the state the simulator reported", () => {
    const state = fold([
      { type: "state", status: "connected" },
      { type: "customization", revision: 1, customization: customization() },
    ]);
    expect(state.revision).toBe(1);
    expect(state.authoritative?.appearance.face).toBe("hybrid");
    expect(state.notice).toBeNull();
  });

  it("reports a change the simulator made on its own", () => {
    const state = fold([
      { type: "state", status: "connected" },
      { type: "customization", revision: 1, customization: customization() },
      { type: "customization", revision: 2, customization: customization("dashboard") },
    ]);
    expect(state.revision).toBe(2);
    expect(state.notice?.tone).toBe("warn");
    expect(state.notice?.text).toContain("changed the watch on its own");
  });

  it("does not report the state that answers its own request as a remote change", () => {
    const state = fold([
      { type: "state", status: "connected" },
      { type: "customization", revision: 1, customization: customization() },
      { type: "request-started", kind: "apply" },
      { type: "customization", revision: 2, customization: customization("classic") },
    ]);
    expect(state.notice).toBeNull();
  });

  it("does not report the same revision again as a remote change", () => {
    const state = fold([
      { type: "state", status: "connected" },
      { type: "customization", revision: 1, customization: customization() },
      { type: "customization", revision: 1, customization: customization() },
    ]);
    expect(state.notice).toBeNull();
  });

  it("clears the changes when the simulator accepts them", () => {
    const state = fold([
      { type: "state", status: "connected" },
      { type: "customization", revision: 1, customization: customization() },
      { type: "edit", edits: setEdit(emptyEdits(), "face", "classic") },
      { type: "request-started", kind: "apply" },
      { type: "request-identified", kind: "apply", requestId: 7 },
      { type: "customization", revision: 2, customization: customization("classic") },
      { type: "ack", requestId: 7, ok: true, revision: 2 },
    ]);
    expect(state.edits).toEqual(emptyEdits());
    expect(state.notice?.tone).toBe("ok");
    expect(state.notice?.text).toContain("revision 2");
    expect(state.pending).toBeNull();
  });

  it("keeps the changes when the simulator refuses them as stale", () => {
    const edits = setEdit(emptyEdits(), "face", "classic");
    const state = fold([
      { type: "state", status: "connected" },
      { type: "customization", revision: 1, customization: customization() },
      { type: "edit", edits },
      { type: "request-started", kind: "apply" },
      { type: "request-identified", kind: "apply", requestId: 7 },
      { type: "customization", revision: 5, customization: customization("dashboard") },
      { type: "ack", requestId: 7, ok: false, revision: 5, code: "conflict" },
    ]);
    expect(state.edits).toEqual(edits);
    expect(state.notice?.tone).toBe("warn");
    expect(state.notice?.text).toContain("revision 5");
    expect(state.notice?.text).toContain("Apply again");
    expect(state.revision).toBe(5);
  });

  it("keeps the changes when the simulator refuses them for another reason", () => {
    const edits = setEdit(emptyEdits(), "face", "classic");
    const state = fold([
      { type: "state", status: "connected" },
      { type: "customization", revision: 1, customization: customization() },
      { type: "edit", edits },
      { type: "request-started", kind: "apply" },
      { type: "request-identified", kind: "apply", requestId: 7 },
      { type: "ack", requestId: 7, ok: false, code: "draft_active" },
    ]);
    expect(state.edits).toEqual(edits);
    expect(state.notice?.tone).toBe("error");
    expect(state.notice?.text).toContain("draft");
  });

  it("keeps the changes when a sync is answered", () => {
    const edits = setEdit(emptyEdits(), "face", "classic");
    const state = fold([
      { type: "state", status: "connected" },
      { type: "customization", revision: 1, customization: customization() },
      { type: "edit", edits },
      { type: "request-started", kind: "sync" },
      { type: "request-identified", kind: "sync", requestId: 9 },
      { type: "customization", revision: 1, customization: customization() },
      { type: "ack", requestId: 9, ok: true, revision: 1 },
    ]);
    expect(state.edits).toEqual(edits);
    expect(state.notice?.tone).toBe("ok");
    expect(state.notice?.text).toContain("reported revision 1");
  });

  it("ignores an answer that belongs to another request", () => {
    const edits = setEdit(emptyEdits(), "face", "classic");
    const state = fold([
      { type: "state", status: "connected" },
      { type: "customization", revision: 1, customization: customization() },
      { type: "edit", edits },
      { type: "request-started", kind: "apply" },
      { type: "request-identified", kind: "apply", requestId: 7 },
      { type: "ack", requestId: 8, ok: true, revision: 2 },
    ]);
    expect(state.edits).toEqual(edits);
    expect(state.pending?.requestId).toBe(7);
    expect(state.notice).toBeNull();
  });

  it("takes an answer that arrives before the id is known", () => {
    const edits = setEdit(emptyEdits(), "face", "classic");
    const state = fold([
      { type: "state", status: "connected" },
      { type: "customization", revision: 1, customization: customization() },
      { type: "edit", edits },
      { type: "request-started", kind: "apply" },
      { type: "ack", requestId: 7, ok: true, revision: 2 },
    ]);
    expect(state.edits).toEqual(emptyEdits());
    expect(state.notice?.tone).toBe("ok");
    expect(state.pending).toBeNull();
  });

  it("does not revive a request whose answer already arrived", () => {
    const state = fold([
      { type: "state", status: "connected" },
      { type: "customization", revision: 1, customization: customization() },
      { type: "request-started", kind: "apply" },
      { type: "ack", requestId: 7, ok: true, revision: 2 },
      { type: "request-identified", kind: "apply", requestId: 7 },
    ]);
    expect(state.pending).toBeNull();
  });

  it("forgets the state when a new session starts", () => {
    const state = fold([
      { type: "state", status: "connected" },
      { type: "customization", revision: 4, customization: customization() },
      { type: "state", status: "disconnected" },
    ]);
    expect(state.known).toBe(false);
    expect(state.pending).toBeNull();
    expect(state.authoritative).toEqual(customization());
  });

  it("treats the first state of a new session as the state, not as a change", () => {
    const state = fold([
      { type: "state", status: "connected" },
      { type: "customization", revision: 4, customization: customization() },
      { type: "state", status: "disconnected" },
      { type: "state", status: "connecting" },
      { type: "state", status: "connected" },
      { type: "customization", revision: 4, customization: customization() },
    ]);
    expect(state.notice).toBeNull();
  });

  it("reports a failure of the link", () => {
    const state = fold([{ type: "error", code: "connection_lost", message: "gone" }]);
    expect(state.notice?.tone).toBe("error");
    expect(state.notice?.text).toContain("connection to the simulator was lost");
  });

  it("leaves a conflict to the answer that carries the revision", () => {
    const state = fold([{ type: "error", code: "conflict", message: "stale" }]);
    expect(state.notice).toBeNull();
  });

  it("drops a request that never reached the simulator", () => {
    const state = fold([{ type: "request-started", kind: "apply" }, { type: "request-failed" }]);
    expect(state.pending).toBeNull();
  });

  it("keeps waiting for an answer when the request was never sent", () => {
    const state = fold([
      { type: "request-started", kind: "apply" },
      { type: "request-identified", kind: "apply", requestId: -1 },
    ]);
    expect(state.pending?.requestId).toBeNull();
  });

  it("discards the changes when they are cancelled", () => {
    const state = fold([
      { type: "edit", edits: setSlotEdit(emptyEdits(), 1, "kind", "cycling") },
      { type: "cancel" },
    ]);
    expect(state.edits).toEqual(emptyEdits());
    expect(state.notice?.tone).toBe("info");
  });
});

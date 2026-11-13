import type { DevLinkCustomization, DevLinkStatus } from "./devlink";
import type { WatchEdits } from "./watch-face";
import { emptyEdits } from "./watch-face";

/**
 * The state of the watch customization screen.
 *
 * The screen keeps the state the simulator reported and, on top of it, only
 * the values the person changed. Everything that happens on the link is
 * folded into this state by one reducer, so the screen itself only renders
 * what the reducer decided.
 *
 * The rules are the simulator's own: the state that is shown is the state the
 * simulator holds, a change is sent on top of the newest state, and a change
 * that was refused leaves the person's edits alone.
 */

export type LinkTone = "ok" | "warn" | "error" | "info";

export type LinkNotice = { tone: LinkTone; text: string };

/** The request that is waiting for an answer, if any. */
type Pending = { kind: "apply" | "sync"; requestId: number | null };

export type LinkState = {
  /** Where the link is right now. */
  status: DevLinkStatus;
  /** Why the link is in this state, when the link said so. */
  statusReason: string | null;
  /** The revision of the committed customization the simulator reported. */
  revision: number;
  /** The committed customization the simulator reported. */
  authoritative: DevLinkCustomization | null;
  /** The values the person changed. */
  edits: WatchEdits;
  /** What the screen tells the person. */
  notice: LinkNotice | null;
  /** The request that is waiting for an answer. */
  pending: Pending | null;
  /** Whether the state was read at least once in this session. */
  known: boolean;
};

export type LinkEvent =
  /** The link changed state. */
  | { type: "state"; status: DevLinkStatus; reason?: string }
  /** The simulator reported the committed customization. */
  | { type: "customization"; revision: number; customization: DevLinkCustomization }
  /** The simulator answered a request. */
  | { type: "ack"; requestId: number; ok: boolean; revision?: number; code?: string }
  /** Something went wrong. */
  | { type: "error"; code: string; message: string }
  /** A request was started; its id is not known yet. */
  | { type: "request-started"; kind: "apply" | "sync" }
  /** The id of a started request became known. */
  | { type: "request-identified"; kind: "apply" | "sync"; requestId: number }
  /** A request never reached the simulator. */
  | { type: "request-failed" }
  /** The person changed a value. */
  | { type: "edit"; edits: WatchEdits }
  /** The person discarded the changes. */
  | { type: "cancel" }
  /** The screen tells the person something of its own. */
  | { type: "notice"; notice: LinkNotice };

/** What each failure of the link means for the person using the screen. */
const FAILURES: Record<string, string> = {
  unauthorized: "The simulator refused the session token.",
  not_authenticated: "The simulator did not accept the session yet.",
  conflict: "The simulator has a newer revision than the change was based on.",
  draft_active: "The watch has an open customization draft, so the change was refused.",
  revision_required: "The simulator needs to know which revision the change is based on.",
  invalid: "The simulator refused the change as invalid.",
  storage_failed: "The simulator could not write its settings file, so nothing was changed.",
  timeout: "The simulator did not answer in time.",
  internal: "The simulator hit an internal error.",
  not_connected: "The development link is not connected.",
  state_unknown: "The simulator state is not known yet.",
  connect_failed: "The simulator could not be reached.",
  connection_lost: "The connection to the simulator was lost.",
  queue_full: "Too many requests are waiting for an answer.",
  request_timeout: "The simulator did not answer the request.",
  request_retried: "The simulator did not answer in time, so the request was sent again.",
  retry_failed: "The request could not be sent again.",
  invalid_token: "The development link needs a session token.",
  invalid_host: "The development link host must not be empty.",
  invalid_port: "The development link port must be between 1 and 65535.",
  invalid_change: "The change could not be prepared.",
  frame_too_large: "The change does not fit in one frame.",
  invalid_frame: "The simulator sent an invalid frame.",
  unknown_message: "The simulator sent a message this app does not know.",
  unexpected_reply: "The simulator answered a request this app does not know.",
};

/** What a failure of the link means for the person using the screen. */
export function describeFailure(code: string, fallback?: string): string {
  return FAILURES[code] ?? fallback ?? "The development link reported a problem.";
}

/** The state the screen starts in. */
export function initialLinkState(): LinkState {
  return {
    status: "idle",
    statusReason: null,
    revision: -1,
    authoritative: null,
    edits: emptyEdits(),
    notice: null,
    pending: null,
    known: false,
  };
}

/**
 * The state the link is in, as the screen shows it.
 *
 * The statuses that mean the link has no usable session drop the state that
 * was read before: a new session has to read the state again.
 */
function withoutSession(state: LinkState): LinkState {
  return { ...state, known: false, pending: null };
}

/**
 * Fold one event of the link into the state of the screen.
 *
 * The reducer never guesses: an answer is only used when it belongs to the
 * request that is waiting for it, and a state that the simulator reported is
 * only called a remote change when it really is one.
 */
export function reduceLink(state: LinkState, event: LinkEvent): LinkState {
  switch (event.type) {
    case "state": {
      const next: LinkState = {
        ...state,
        status: event.status,
        statusReason: event.reason ?? null,
      };
      if (event.status === "connected") return next;
      if (
        event.status === "connecting" ||
        event.status === "disconnected" ||
        event.status === "idle" ||
        event.status === "error"
      ) {
        return withoutSession(next);
      }
      return next;
    }

    case "customization": {
      const next: LinkState = {
        ...state,
        known: true,
        revision: event.revision,
        authoritative: event.customization,
      };
      // The first reading of a session is not a change, and the state that
      // answers a request of this screen is not a remote change either.
      if (!state.known || state.pending) return next;
      if (event.revision === state.revision) return next;
      return {
        ...next,
        notice: {
          tone: "warn",
          text: `The simulator changed the watch on its own (revision ${event.revision}).`,
        },
      };
    }

    case "ack": {
      const pending = state.pending;
      if (!pending) return state;
      // The answer belongs to this request only when the ids agree. The id is
      // not known yet when the answer arrives before the call returns.
      if (pending.requestId !== null && pending.requestId !== event.requestId) return state;
      const next: LinkState = { ...state, pending: null };
      if (pending.kind === "sync") {
        // A sync only reads the state; it never changes what the person edited.
        return {
          ...next,
          notice: {
            tone: "ok",
            text: `The simulator reported revision ${event.revision ?? "?"}.`,
          },
        };
      }
      if (event.ok) {
        return {
          ...next,
          edits: emptyEdits(),
          notice: {
            tone: "ok",
            text: `Applied. The simulator is now at revision ${event.revision ?? "?"}.`,
          },
        };
      }
      if (event.code === "conflict") {
        // The changes are kept: the next attempt is sent on top of the state
        // the simulator reported with the refusal.
        return {
          ...next,
          notice: {
            tone: "warn",
            text:
              `The simulator moved on to revision ${event.revision ?? "?"} while you were editing. ` +
              "Your changes are kept — press Apply again to send them.",
          },
        };
      }
      return {
        ...next,
        notice: { tone: "error", text: describeFailure(event.code ?? "internal") },
      };
    }

    case "error": {
      // A conflict is explained by the acknowledgement, which carries the
      // revision the simulator is at.
      if (event.code === "conflict") return state;
      return {
        ...state,
        notice: { tone: "error", text: describeFailure(event.code, event.message) },
      };
    }

    case "request-started":
      return { ...state, pending: { kind: event.kind, requestId: null } };

    case "request-identified": {
      const pending = state.pending;
      // The answer may already have arrived; then there is nothing to identify.
      if (!pending || pending.kind !== event.kind || pending.requestId !== null) return state;
      // A negative id means the request was never sent, so there is no answer
      // to wait for.
      if (event.requestId < 0) return state;
      return { ...state, pending: { ...pending, requestId: event.requestId } };
    }

    case "request-failed":
      return { ...state, pending: null };

    case "edit":
      return { ...state, edits: event.edits };

    case "cancel":
      return {
        ...state,
        edits: emptyEdits(),
        notice: { tone: "info", text: "Your changes were discarded." },
      };

    case "notice":
      return { ...state, notice: event.notice };
  }
}

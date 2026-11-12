import type { ReactNode } from "react";
import { useEffect, useRef, useState } from "react";
import {
  ActivityIndicator,
  Pressable,
  ScrollView,
  StyleSheet,
  Switch,
  Text,
  TextInput,
  useColorScheme,
  View,
} from "react-native";
import type {
  DevLinkAccent,
  DevLinkComplicationKind,
  DevLinkComplicationStyle,
  DevLinkCustomization,
  DevLinkFace,
  DevLinkPalette,
} from "../../lib/devlink";
import {
  devLink,
  devLinkAvailable,
  devLinkLabBuild,
  watchDevLinkAck,
  watchDevLinkCustomization,
  watchDevLinkError,
  watchDevLinkState,
} from "../../lib/devlink";
import type { Choice } from "../../lib/watch-face";
import {
  ACCENT_CHOICES,
  buildPatch,
  emptyEdits,
  FACE_CHOICES,
  hasEdits,
  KIND_CHOICES,
  PALETTE_CHOICES,
  SLOT_COUNT,
  STYLE_CHOICES,
  setEdit,
  setSlotEdit,
  slotField,
} from "../../lib/watch-face";
import type { LinkEvent, LinkState } from "../../lib/watch-link-state";
import { initialLinkState, reduceLink } from "../../lib/watch-link-state";

/**
 * The watch customization screen of the Charlie Wrist Lab.
 *
 * It talks to the Rust WatchOS simulator over the development link that the
 * `rakazo-devlink` module implements. It configures the simulator and nothing
 * else: the physical watch is never written to, the Charlie assistant is never
 * contacted, and every value is stored by the simulator's own settings file.
 *
 * The screen keeps the state the simulator reported and, on top of it, only
 * the values that were changed. A change is therefore always sent on top of
 * the newest state the simulator holds, which is what makes a stale revision
 * recoverable instead of fatal. All of that is decided by the reducer in
 * `lib/watch-link-state.ts`, so this file only renders what it decided.
 */

/** The port the simulator's development link listens on by default. */
const DEFAULT_PORT = "4599";

/** The host the ADB reverse tunnel forwards to. */
const DEFAULT_HOST = "127.0.0.1";

/** The shortest session token the simulator accepts. */
const MIN_TOKEN_LENGTH = 16;

/** How much one press of the glow control changes the glow. */
const GLOW_STEP = 5;

/** The placeholders used before the simulator has reported anything. */
const PLACEHOLDER_STATE: DevLinkCustomization = {
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

export default function WatchFaceScreen() {
  const dark = useColorScheme() !== "light";
  const foreground = dark ? "#F5FAFA" : "#142A30";
  const muted = dark ? "#AAB8BF" : "#51666E";
  const surface = dark ? "#1B252B" : "#FFFFFF";
  const background = dark ? "#0B1318" : "#F2F7F8";
  const line = dark ? "#33464D" : "#D0E1E3";

  const [labBuild, setLabBuild] = useState(false);
  const [link, setLink] = useState<LinkState>(initialLinkState);
  const [token, setToken] = useState("");
  const [host, setHost] = useState(DEFAULT_HOST);
  const [port, setPort] = useState(DEFAULT_PORT);
  const [busy, setBusy] = useState(false);

  // The link asks for the state again once the simulator answered the
  // handshake, so the screen starts from the newest state it holds.
  const syncOnConnectRef = useRef(false);

  const dispatch = (event: LinkEvent) => setLink((current) => reduceLink(current, event));

  useEffect(() => {
    setLabBuild(devLinkLabBuild());
  }, []);

  useEffect(() => {
    const stateSub = watchDevLinkState((event) => {
      dispatch({ type: "state", status: event.status, reason: event.reason });
      if (event.status === "connected" && syncOnConnectRef.current) {
        // The handshake already carried the authoritative state; asking for it
        // again makes sure the screen works from the newest one.
        syncOnConnectRef.current = false;
        void devLink.requestSync().catch(() => undefined);
      }
    });

    const customizationSub = watchDevLinkCustomization((event) => {
      dispatch({
        type: "customization",
        revision: event.revision,
        customization: event.customization,
      });
    });

    const ackSub = watchDevLinkAck((event) => {
      dispatch({
        type: "ack",
        requestId: event.requestId,
        ok: event.ok,
        revision: event.revision,
        code: event.code,
      });
    });

    const errorSub = watchDevLinkError((event) => {
      dispatch({ type: "error", code: event.code, message: event.message });
    });

    return () => {
      stateSub.remove();
      customizationSub.remove();
      ackSub.remove();
      errorSub.remove();
    };
  }, []);

  const connected = link.status === "connected";
  const changed = hasEdits(link.edits);
  const available = devLinkAvailable();
  const glowValue = link.edits.glow ?? link.authoritative?.appearance.glow ?? 60;

  async function run(operation: () => Promise<void>) {
    setBusy(true);
    try {
      await operation();
    } catch (cause) {
      // The request never reached the simulator, so nothing is waiting for it.
      dispatch({ type: "request-failed" });
      dispatch({
        type: "notice",
        notice: {
          tone: "error",
          text: cause instanceof Error ? cause.message : "The development link reported a problem.",
        },
      });
    } finally {
      setBusy(false);
    }
  }

  async function connect() {
    const parsed = Number(port.trim());
    if (!Number.isInteger(parsed) || parsed <= 0 || parsed > 65535) {
      dispatch({
        type: "notice",
        notice: { tone: "error", text: "The port must be a number between 1 and 65535." },
      });
      return;
    }
    if (token.trim().length < MIN_TOKEN_LENGTH) {
      dispatch({
        type: "notice",
        notice: {
          tone: "error",
          text: `The simulator needs a session token of at least ${MIN_TOKEN_LENGTH} characters.`,
        },
      });
      return;
    }
    syncOnConnectRef.current = true;
    await run(async () => {
      await devLink.connect({ host: host.trim() || DEFAULT_HOST, port: parsed, token });
      dispatch({
        type: "notice",
        notice: { tone: "info", text: "Waiting for the simulator to answer…" },
      });
    });
  }

  async function sync() {
    if (!connected) return;
    dispatch({ type: "request-started", kind: "sync" });
    await run(async () => {
      const requestId = await devLink.requestSync();
      dispatch({ type: "request-identified", kind: "sync", requestId });
      dispatch({
        type: "notice",
        notice: { tone: "info", text: "Asking the simulator for its current settings…" },
      });
    });
  }

  async function apply() {
    if (!link.authoritative) return;
    const patch = buildPatch(link.authoritative, link.edits);
    if (!patch) {
      dispatch({
        type: "notice",
        notice: { tone: "info", text: "There is nothing to apply." },
      });
      return;
    }
    dispatch({ type: "request-started", kind: "apply" });
    await run(async () => {
      const requestId = await devLink.applyCustomization(patch);
      dispatch({ type: "request-identified", kind: "apply", requestId });
      dispatch({
        type: "notice",
        notice: { tone: "info", text: "Sending your changes to the simulator…" },
      });
    });
  }

  const button = (label: string, onPress: () => void, disabled = false) => (
    <Pressable
      accessibilityRole="button"
      accessibilityState={{ disabled: busy || disabled }}
      onPress={onPress}
      disabled={busy || disabled}
      style={({ pressed }) => [
        styles.button,
        { opacity: busy || disabled ? 0.4 : pressed ? 0.75 : 1 },
      ]}
    >
      <Text style={styles.buttonText}>{label}</Text>
    </Pressable>
  );

  const card = (title: string, children: ReactNode) => (
    <View style={[styles.card, { backgroundColor: surface }]}>
      <Text style={[styles.section, { color: foreground }]}>{title}</Text>
      {children}
    </View>
  );

  const choice = <T extends string>(
    options: readonly Choice<T>[],
    selected: T,
    onSelect: (value: T) => void,
    disabled: boolean,
  ) => (
    <View style={styles.choices}>
      {options.map((option) => {
        const active = option.value === selected;
        return (
          <Pressable
            key={option.value}
            accessibilityRole="button"
            accessibilityState={{ selected: active, disabled }}
            disabled={disabled}
            onPress={() => onSelect(option.value)}
            style={({ pressed }) => [
              styles.choice,
              {
                backgroundColor: active ? "#189F9C" : "transparent",
                borderColor: active ? "#189F9C" : line,
                opacity: disabled ? 0.4 : pressed ? 0.75 : 1,
              },
            ]}
          >
            <Text
              style={[
                styles.choiceText,
                { color: active ? "#FFFFFF" : foreground, fontWeight: active ? "700" : "500" },
              ]}
            >
              {option.label}
            </Text>
          </Pressable>
        );
      })}
    </View>
  );

  const toggle = (
    label: string,
    detail: string,
    value: boolean,
    onChange: (next: boolean) => void,
  ) => (
    <View style={styles.toggle}>
      <View style={styles.toggleText}>
        <Text style={{ color: foreground, fontWeight: "600" }}>{label}</Text>
        <Text style={{ color: muted, fontSize: 12 }}>{detail}</Text>
      </View>
      <Switch
        accessibilityLabel={label}
        value={value}
        onValueChange={onChange}
        trackColor={{ true: "#189F9C", false: line }}
        thumbColor="#FFFFFF"
      />
    </View>
  );

  const stateLabel = connected
    ? `Connected · revision ${link.revision}`
    : link.status === "idle"
      ? "Not connected"
      : link.status.replaceAll("-", " ");

  return (
    <ScrollView style={{ backgroundColor: background }} contentContainerStyle={styles.content}>
      <View style={[styles.hero, { backgroundColor: surface }]}>
        <Text style={[styles.eyebrow, { color: "#189F9C" }]}>CHARLIE WRIST LAB</Text>
        <Text style={[styles.title, { color: foreground }]}>Watch face</Text>
        <Text style={[styles.body, { color: muted }]}>
          Configure the Rust WatchOS simulator. The simulator keeps the settings and writes them to
          its own file; the physical watch is never written to.
        </Text>
        <View
          style={[
            styles.stateChip,
            { backgroundColor: connected ? "#103C3A" : dark ? "#303940" : "#E3EAED" },
          ]}
        >
          <View style={[styles.dot, { backgroundColor: connected ? "#64E0B7" : "#93A4AE" }]} />
          <Text style={{ color: connected ? "#8DF4D3" : foreground, fontWeight: "700" }}>
            {stateLabel}
          </Text>
        </View>
        {link.statusReason ? (
          <Text style={[styles.body, { color: muted }]}>{link.statusReason}</Text>
        ) : null}
      </View>

      {!available ? (
        card(
          "1. Simulator link",
          <Text style={[styles.body, { color: foreground }]}>
            This screen needs an Android build that carries the development link.
          </Text>,
        )
      ) : !labBuild ? (
        card(
          "1. Simulator link",
          <Text style={[styles.body, { color: foreground }]}>
            The development link only works in the Charlie Wrist Lab build.
          </Text>,
        )
      ) : (
        <>
          {card(
            "1. Simulator link",
            <>
              <Text style={[styles.body, { color: muted }]}>
                The simulator listens on the loopback interface of the computer that runs it. On a
                device the tunnel is made with `adb reverse tcp:4599 tcp:4599`.
              </Text>
              <Text style={[styles.label, { color: foreground }]}>Host</Text>
              <TextInput
                accessibilityLabel="Host"
                value={host}
                onChangeText={setHost}
                autoCapitalize="none"
                autoCorrect={false}
                placeholder={DEFAULT_HOST}
                placeholderTextColor={muted}
                style={[styles.input, { color: foreground, borderColor: line }]}
              />
              <Text style={[styles.label, { color: foreground }]}>Port</Text>
              <TextInput
                accessibilityLabel="Port"
                value={port}
                onChangeText={setPort}
                keyboardType="number-pad"
                placeholder={DEFAULT_PORT}
                placeholderTextColor={muted}
                style={[styles.input, { color: foreground, borderColor: line }]}
              />
              <Text style={[styles.label, { color: foreground }]}>Session token</Text>
              <TextInput
                accessibilityLabel="Session token"
                value={token}
                onChangeText={setToken}
                secureTextEntry
                autoCapitalize="none"
                autoCorrect={false}
                autoComplete="off"
                placeholder="the token the simulator was started with"
                placeholderTextColor={muted}
                style={[styles.input, { color: foreground, borderColor: line }]}
              />
              <Text style={[styles.body, { color: muted, fontSize: 12 }]}>
                The token stays in memory while this screen is open. It is never written to disk and
                never logged.
              </Text>
              {button(connected ? "Reconnect" : "Connect", () => void connect(), busy)}
              {button("Disconnect", () => void run(() => devLink.disconnect()), !connected)}
              {button("Sync now", () => void sync(), !connected)}
            </>,
          )}

          {card(
            "2. Watch face",
            <>
              <Text style={[styles.body, { color: muted }]}>
                The four faces the simulator draws.
              </Text>
              {choice(
                FACE_CHOICES,
                link.edits.face ?? link.authoritative?.appearance.face ?? "hybrid",
                (value) =>
                  dispatch({
                    type: "edit",
                    edits: setEdit(link.edits, "face", value as DevLinkFace),
                  }),
                !connected,
              )}
            </>,
          )}

          {card(
            "3. Colour palette",
            <>
              <Text style={[styles.body, { color: muted }]}>
                The five palettes the simulator draws.
              </Text>
              {choice(
                PALETTE_CHOICES,
                link.edits.palette ?? link.authoritative?.appearance.palette ?? "cyan_violet",
                (value) =>
                  dispatch({
                    type: "edit",
                    edits: setEdit(link.edits, "palette", value as DevLinkPalette),
                  }),
                !connected,
              )}
            </>,
          )}

          {card(
            "4. Display",
            <>
              <Text style={[styles.body, { color: muted }]}>Glow</Text>
              <View style={styles.glow}>
                <Pressable
                  accessibilityRole="button"
                  accessibilityLabel="Less glow"
                  disabled={!connected}
                  onPress={() =>
                    dispatch({
                      type: "edit",
                      edits: setEdit(link.edits, "glow", Math.max(0, glowValue - GLOW_STEP)),
                    })
                  }
                  style={[styles.glowButton, { borderColor: line, opacity: connected ? 1 : 0.4 }]}
                >
                  <Text style={{ color: foreground, fontSize: 18 }}>−</Text>
                </Pressable>
                <View style={styles.glowBar}>
                  <View
                    style={[
                      styles.glowFill,
                      {
                        width: `${glowValue}%`,
                        backgroundColor: "#189F9C",
                        opacity: connected ? 1 : 0.4,
                      },
                    ]}
                  />
                </View>
                <Text style={[styles.glowValue, { color: foreground }]}>{glowValue}</Text>
                <Pressable
                  accessibilityRole="button"
                  accessibilityLabel="More glow"
                  disabled={!connected}
                  onPress={() =>
                    dispatch({
                      type: "edit",
                      edits: setEdit(link.edits, "glow", Math.min(100, glowValue + GLOW_STEP)),
                    })
                  }
                  style={[styles.glowButton, { borderColor: line, opacity: connected ? 1 : 0.4 }]}
                >
                  <Text style={{ color: foreground, fontSize: 18 }}>+</Text>
                </Pressable>
              </View>
              {toggle(
                "Show date",
                "The date under the time",
                link.edits.show_date ?? link.authoritative?.appearance.show_date ?? true,
                (next) => dispatch({ type: "edit", edits: setEdit(link.edits, "show_date", next) }),
              )}
              {toggle(
                "Show seconds",
                "The second hand and the seconds in the time",
                link.edits.show_seconds ?? link.authoritative?.appearance.show_seconds ?? false,
                (next) =>
                  dispatch({ type: "edit", edits: setEdit(link.edits, "show_seconds", next) }),
              )}
              {toggle(
                "Show complications",
                "The three slots under the time",
                link.edits.show_complications ??
                  link.authoritative?.appearance.show_complications ??
                  true,
                (next) =>
                  dispatch({
                    type: "edit",
                    edits: setEdit(link.edits, "show_complications", next),
                  }),
              )}
            </>,
          )}

          {card(
            "5. Complications",
            <>
              <Text style={[styles.body, { color: muted }]}>
                The three slots the watch shows under the time.
              </Text>
              {Array.from({ length: SLOT_COUNT }, (_, slot) => (
                <View key={slot} style={[styles.slot, { borderColor: line }]}>
                  <Text style={[styles.slotTitle, { color: foreground }]}>Slot {slot + 1}</Text>
                  <Text style={[styles.label, { color: muted }]}>Kind</Text>
                  {choice(
                    KIND_CHOICES,
                    slotField(link.authoritative ?? PLACEHOLDER_STATE, link.edits, slot, "kind"),
                    (value) =>
                      dispatch({
                        type: "edit",
                        edits: setSlotEdit(
                          link.edits,
                          slot,
                          "kind",
                          value as DevLinkComplicationKind,
                        ),
                      }),
                    !connected,
                  )}
                  <Text style={[styles.label, { color: muted }]}>Accent</Text>
                  {choice(
                    ACCENT_CHOICES,
                    slotField(link.authoritative ?? PLACEHOLDER_STATE, link.edits, slot, "accent"),
                    (value) =>
                      dispatch({
                        type: "edit",
                        edits: setSlotEdit(link.edits, slot, "accent", value as DevLinkAccent),
                      }),
                    !connected,
                  )}
                  <Text style={[styles.label, { color: muted }]}>Style</Text>
                  {choice(
                    STYLE_CHOICES,
                    slotField(link.authoritative ?? PLACEHOLDER_STATE, link.edits, slot, "style"),
                    (value) =>
                      dispatch({
                        type: "edit",
                        edits: setSlotEdit(
                          link.edits,
                          slot,
                          "style",
                          value as DevLinkComplicationStyle,
                        ),
                      }),
                    !connected,
                  )}
                  {toggle(
                    "Visible",
                    "Whether the watch shows this slot",
                    slotField(link.authoritative ?? PLACEHOLDER_STATE, link.edits, slot, "visible"),
                    (next) =>
                      dispatch({
                        type: "edit",
                        edits: setSlotEdit(link.edits, slot, "visible", next),
                      }),
                  )}
                </View>
              ))}
            </>,
          )}

          {card(
            "6. Clock and motion",
            <>
              {toggle(
                "24-hour clock",
                "The watch shows the time on a 24-hour scale",
                link.edits.use_24h ?? link.authoritative?.use_24h ?? true,
                (next) => dispatch({ type: "edit", edits: setEdit(link.edits, "use_24h", next) }),
              )}
              {toggle(
                "Reduced motion",
                "The watch moves less",
                link.edits.reduce_motion ?? link.authoritative?.reduce_motion ?? false,
                (next) =>
                  dispatch({ type: "edit", edits: setEdit(link.edits, "reduce_motion", next) }),
              )}
            </>,
          )}

          {link.notice ? (
            <Text
              selectable
              style={[
                styles.notice,
                {
                  color:
                    link.notice.tone === "error"
                      ? "#CE4E4E"
                      : link.notice.tone === "warn"
                        ? "#B4762A"
                        : link.notice.tone === "ok"
                          ? "#2E8B70"
                          : muted,
                },
              ]}
            >
              {link.notice.text}
            </Text>
          ) : null}

          <View style={styles.actions}>
            {button("Cancel", () => dispatch({ type: "cancel" }), !changed)}
            {button("Apply to simulator", () => void apply(), !changed || !connected)}
          </View>
          <Text style={[styles.footnote, { color: muted }]}>
            Apply sends only the values you changed to the simulator, which stores them in its own
            settings file. Cancel discards them.
          </Text>
        </>
      )}
      {busy ? <ActivityIndicator style={styles.spinner} color="#189F9C" /> : null}
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  content: { padding: 18, gap: 14, paddingBottom: 44 },
  hero: { padding: 22, borderRadius: 22, gap: 12 },
  eyebrow: { fontWeight: "800", letterSpacing: 1.4, fontSize: 12 },
  title: { fontSize: 29, fontWeight: "800" },
  section: { fontSize: 18, fontWeight: "800", marginBottom: 6 },
  body: { fontSize: 14, lineHeight: 21 },
  card: { padding: 18, gap: 10, borderRadius: 18 },
  stateChip: {
    flexDirection: "row",
    alignSelf: "flex-start",
    alignItems: "center",
    padding: 10,
    borderRadius: 12,
    gap: 8,
  },
  dot: { width: 9, height: 9, borderRadius: 9 },
  label: { fontSize: 13, fontWeight: "700", marginTop: 4 },
  input: {
    borderWidth: 1,
    borderRadius: 12,
    paddingHorizontal: 13,
    paddingVertical: 11,
    fontSize: 14,
  },
  choices: { flexDirection: "row", flexWrap: "wrap", gap: 8 },
  choice: { borderWidth: 1, borderRadius: 999, paddingVertical: 8, paddingHorizontal: 13 },
  choiceText: { fontSize: 13 },
  toggle: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    gap: 12,
    paddingVertical: 4,
  },
  toggleText: { flex: 1, gap: 2 },
  glow: { flexDirection: "row", alignItems: "center", gap: 10 },
  glowButton: {
    width: 38,
    height: 38,
    borderWidth: 1,
    borderRadius: 12,
    alignItems: "center",
    justifyContent: "center",
  },
  glowBar: { flex: 1, height: 8, borderRadius: 8, backgroundColor: "#2A3A41", overflow: "hidden" },
  glowFill: { height: 8, borderRadius: 8 },
  glowValue: { fontSize: 15, fontWeight: "700", width: 34, textAlign: "right" },
  slot: { borderWidth: 1, borderRadius: 14, padding: 13, gap: 8, marginTop: 4 },
  slotTitle: { fontSize: 15, fontWeight: "700" },
  button: {
    backgroundColor: "#167D7C",
    paddingVertical: 13,
    paddingHorizontal: 18,
    borderRadius: 12,
    alignItems: "center",
  },
  buttonText: { color: "#FFFFFF", fontWeight: "700", fontSize: 14 },
  actions: { flexDirection: "row", gap: 10 },
  notice: { fontSize: 13, lineHeight: 19, paddingHorizontal: 4 },
  spinner: { marginTop: 4 },
  footnote: { fontSize: 12, lineHeight: 17, paddingHorizontal: 4 },
});

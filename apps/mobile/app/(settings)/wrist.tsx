import { useEffect, useState } from "react";
import {
  ActivityIndicator,
  Platform,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  View,
  useColorScheme,
} from "react-native";
import type { WristDevice, WristEvent, WristState } from "../../lib/wrist";
import {
  requestWristBluetoothPermissions,
  watchWristDevices,
  watchWristEvents,
  watchWristState,
  wrist,
  wristAvailable,
} from "../../lib/wrist";

const initialState: WristState = { status: "disconnected" };

export default function CharlieWristScreen() {
  const dark = useColorScheme() !== "light";
  const foreground = dark ? "#F5FAFA" : "#142A30";
  const muted = dark ? "#AAB8BF" : "#51666E";
  const surface = dark ? "#1B252B" : "#FFFFFF";
  const background = dark ? "#0B1318" : "#F2F7F8";

  const [devices, setDevices] = useState<WristDevice[]>([]);
  const [connection, setConnection] = useState<WristState>(initialState);
  const [battery, setBattery] = useState<number | null>(null);
  const [findPhone, setFindPhone] = useState<boolean | null>(null);
  const [notificationAccepted, setNotificationAccepted] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const available = Platform.OS === "android" && wristAvailable();
  const ready = connection.status === "ready";

  useEffect(() => {
    const deviceSub = watchWristDevices((device) => {
      setDevices((existing) =>
        existing.some((value) => value.address === device.address)
          ? existing
          : [...existing, device].slice(0, 10),
      );
    });
    const stateSub = watchWristState((next) => {
      setConnection(next);
      if (next.status === "error") setError(next.reason ?? "Bluetooth operation failed");
    });
    const eventSub = watchWristEvents((event: WristEvent) => {
      if (event.type === "battery") setBattery(event.percent);
      else if (event.type === "find-phone") setFindPhone(event.active);
      else if (event.type === "notification-ack") setNotificationAccepted(event.accepted);
    });
    return () => {
      deviceSub.remove();
      stateSub.remove();
      eventSub.remove();
      // Prototype mode: no hidden/background watch connection after leaving this screen.
      if (wristAvailable()) {
        void wrist.stopScan().catch(() => undefined);
        void wrist.disconnect().catch(() => undefined);
      }
    };
  }, []);

  async function run(operation: () => Promise<void>) {
    setBusy(true);
    setError(null);
    try {
      await operation();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Bluetooth operation failed");
    } finally {
      setBusy(false);
    }
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

  return (
    <ScrollView style={{ backgroundColor: background }} contentContainerStyle={styles.content}>
      <View style={[styles.hero, { backgroundColor: surface }]}>
        <Text style={[styles.eyebrow, { color: "#189F9C" }]}>CHARLIE WRIST LAB</Text>
        <Text style={[styles.title, { color: foreground }]}>Lunar Discovery</Text>
        <Text style={[styles.body, { color: muted }]}>
          Private, direct Bluetooth control for your KaHa WA49V3.
          This prototype does not modify your watch firmware.
        </Text>
        <View style={[styles.stateChip, { backgroundColor: ready ? "#103C3A" : dark ? "#303940" : "#E3EAED" }]}>
          <View style={[styles.dot, { backgroundColor: ready ? "#64E0B7" : "#93A4AE" }]} />
          <Text style={{ color: ready ? "#8DF4D3" : foreground, fontWeight: "700" }}>
            {ready ? "Connected and verified" : connection.status.replaceAll("-", " ")}
          </Text>
        </View>
        {battery !== null ? (
          <Text style={[styles.battery, { color: foreground }]}>Battery: {battery}%</Text>
        ) : null}
      </View>

      {!available ? (
        <View style={[styles.card, { backgroundColor: surface }]}>
          <Text style={[styles.body, { color: foreground }]}>
            Charlie Wrist requires an Android APK containing the native BLE module.
          </Text>
        </View>
      ) : (
        <>
          <View style={[styles.card, { backgroundColor: surface }]}>
            <Text style={[styles.section, { color: foreground }]}>1. Connect your watch</Text>
            <Text style={[styles.body, { color: muted }]}>
              Disconnect nRF Connect and force-stop boAt Crest before connecting.
              Scan and choose your watch below.
            </Text>
            {button("Scan for Lunar Discovery", () =>
              void run(async () => {
                setDevices([]);
                if (!(await requestWristBluetoothPermissions())) {
                  throw new Error("Nearby Devices permission was not granted");
                }
                await wrist.startScan();
              }), ready)}
            {devices.map((device) => (
              <Pressable
                key={device.address}
                accessibilityRole="button"
                disabled={busy || ready}
                onPress={() => void run(() => wrist.connect(device.address))}
                style={[styles.device, { borderColor: dark ? "#4D676C" : "#D0E1E3" }]}
              >
                <Text style={{ color: foreground, fontWeight: "700" }}>{device.name}</Text>
                <Text style={{ color: muted, fontSize: 12 }}>
                  Tap to connect · address stays on this phone
                </Text>
              </Pressable>
            ))}
            {button("Disconnect", () => void run(() => wrist.disconnect()),
              connection.status === "disconnected")}
          </View>

          <View style={[styles.card, { backgroundColor: surface }]}>
            <Text style={[styles.section, { color: foreground }]}>2. Try verified commands</Text>
            <Text style={[styles.body, { color: muted }]}>
              Actions unlock after the watch reports model WA49V3.
            </Text>
            {button("Read watch battery", () => void run(() => wrist.readBattery()), !ready)}
            {button("Vibrate once", () => void run(() => wrist.vibrate()), !ready)}
            {button('Send "Charlie: Hello"', () => {
              setNotificationAccepted(null);
              void run(() => wrist.sendNotification("Charlie", "Hello"));
            }, !ready)}
            {notificationAccepted !== null ? (
              <Text style={[styles.body, { color: muted }]}>
                Notification acknowledgement: {notificationAccepted ? "accepted" : "not accepted"}
              </Text>
            ) : null}
          </View>

          <View style={[styles.card, { backgroundColor: surface }]}>
            <Text style={[styles.section, { color: foreground }]}>3. Watch → phone</Text>
            <Text style={[styles.body, { color: muted }]}>
              Use Find My Phone on your watch while connected.
            </Text>
            <Text style={[styles.body, { color: foreground }]}>
              Last event: {findPhone === null ? "None" : findPhone ? "Find My Phone activated" : "Find My Phone inactive"}
            </Text>
            <Text style={[styles.body, { color: muted }]}>
              Only known event types are shown. Raw Bluetooth packets and device addresses are not uploaded.
            </Text>
          </View>
        </>
      )}
      {busy ? <ActivityIndicator style={styles.spinner} color="#189F9C" /> : null}
      {error ? <Text selectable style={styles.error}>{error}</Text> : null}
      <Text style={[styles.footnote, { color: muted }]}>
        Experimental foreground-only integration. Auto-reconnect, voice and Charlie server routing
        are not enabled in this build.
      </Text>
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
  stateChip: { flexDirection: "row", alignSelf: "flex-start", alignItems: "center", padding: 10, borderRadius: 12, gap: 8 },
  dot: { width: 9, height: 9, borderRadius: 9 },
  battery: { fontSize: 19, fontWeight: "700" },
  device: { padding: 13, borderWidth: 1, borderRadius: 12, gap: 4 },
  button: { backgroundColor: "#167D7C", paddingVertical: 13, paddingHorizontal: 18, borderRadius: 12, alignItems: "center" },
  buttonText: { color: "#FFFFFF", fontWeight: "700", fontSize: 14 },
  spinner: { marginTop: 4 },
  error: { color: "#CE4E4E", fontSize: 13 },
  footnote: { fontSize: 12, lineHeight: 17, paddingHorizontal: 4 },
});

import { useLingui } from "@lingui/react/macro";
import { useRouter } from "expo-router";
import { typeScale } from "@rakazo/ui-tokens";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  ActivityIndicator,
  Platform,
  ScrollView,
  StyleSheet,
  Text,
  View,
} from "react-native";
import { NativeActionButton } from "../../components/native-action-button";
import {
  SettingsFooter,
  SettingsGroup,
  SettingsRow,
  useStackedSettings,
} from "../../components/settings-group";
import { mobileTokens } from "../../lib/appearance";
import { devLinkLabBuild } from "../../lib/devlink";
import { native, useThemedStyles } from "../../lib/native";
import type { WristDevice, WristDeviceInfo, WristEvent, WristState } from "../../lib/wrist";
import {
  requestWristBluetoothPermissions,
  watchWristDeviceInfo,
  watchWristDevices,
  watchWristEvents,
  watchWristState,
  wrist,
  wristAvailable,
} from "../../lib/wrist";

const initialState: WristState = { status: "disconnected" };

export default function CharlieWristScreen() {
  const { t } = useLingui();
  const styles = useThemedStyles(createWristStyles);
  const stacked = useStackedSettings();
  const router = useRouter();

  const [devices, setDevices] = useState<WristDevice[]>([]);
  const [connection, setConnection] = useState<WristState>(initialState);
  const [deviceInfo, setDeviceInfo] = useState<WristDeviceInfo | null>(null);
  const [uartBattery, setUartBattery] = useState<{
    percent: number;
    rawHex: string;
    timestamp: string;
  } | null>(null);
  const [findPhone, setFindPhone] = useState<boolean | null>(null);
  const [notificationAccepted, setNotificationAccepted] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const lastStateGeneration = useRef(0);

  const available = Platform.OS === "android" && wristAvailable();
  const labBuild = Platform.OS === "android" && devLinkLabBuild();
  const ready = connection.status === "ready";
  const isReadOnly = connection.status === "read-only-ready" || connection.status === "inspected";

  const applyState = useCallback(
    (next: WristState) => {
      if (next.generation !== undefined) {
        if (next.generation < lastStateGeneration.current) {
          return;
        }
        lastStateGeneration.current = next.generation;
      }
      setConnection((prev) => ({
        ...prev,
        ...next,
        persistentEnabled:
          typeof next.persistentEnabled === "boolean"
            ? next.persistentEnabled
            : prev.persistentEnabled,
      }));
      if (next.status === "error") {
        setError(next.reason ?? t`Bluetooth operation failed`);
      } else if (next.status === "disconnected") {
        setDeviceInfo(null);
        setUartBattery(null);
        setFindPhone(null);
        setNotificationAccepted(null);
      }
    },
    [t],
  );

  useEffect(() => {
    let mounted = true;

    const deviceSub = watchWristDevices((device) => {
      if (!mounted) return;
      setDevices((existing) => {
        if (existing.some((value) => value.address === device.address)) return existing;
        return [...existing, device]
          .sort((a, b) => Number(b.likelyWatch) - Number(a.likelyWatch) || b.rssi - a.rssi)
          .slice(0, 30);
      });
    });

    const stateSub = watchWristState((next) => {
      if (!mounted) return;
      applyState(next);
    });

    const infoSub = watchWristDeviceInfo((info) => {
      if (!mounted) return;
      setDeviceInfo(info);
    });

    const eventSub = watchWristEvents((event: WristEvent) => {
      if (!mounted) return;
      if (event.type === "battery") {
        const time = new Date(event.timestamp ?? Date.now()).toLocaleTimeString();
        setUartBattery({
          percent: event.percent,
          rawHex: event.rawHex ?? "",
          timestamp: time,
        });
      } else if (event.type === "find-phone") {
        setFindPhone(event.active);
      } else if (event.type === "notification-ack") {
        setNotificationAccepted(event.accepted);
      }
    });

    if (wristAvailable()) {
      wrist
        .getState()
        .then((current) => {
          if (mounted && current) {
            applyState(current);
          }
        })
        .catch(() => undefined);

      wrist
        .getDeviceInfo()
        .then((info) => {
          if (mounted && info) {
            setDeviceInfo(info);
          }
        })
        .catch(() => undefined);
    }

    return () => {
      mounted = false;
      deviceSub.remove();
      stateSub.remove();
      infoSub.remove();
      eventSub.remove();
      if (wristAvailable()) {
        void wrist.stopScan().catch(() => undefined);
      }
    };
  }, [applyState]);

  async function run(operation: () => Promise<void>) {
    setBusy(true);
    setError(null);
    try {
      await operation();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : t`Bluetooth operation failed`);
    } finally {
      setBusy(false);
    }
  }

  function formatStatus(status: string): string {
    switch (status) {
      case "ready":
        return t`Connected (Active & Verified)`;
      case "read-only-ready":
        return t`Connected (Read-Only Mode)`;
      case "inspected":
        return t`Inspection Complete (Clean Disconnect)`;
      case "reading-info":
        return t`Reading Device Info...`;
      case "verifying-model":
        return t`Verifying Model...`;
      case "discovering":
        return t`Discovering Services...`;
      case "connecting":
        return t`Connecting...`;
      case "scanning":
        return t`Scanning...`;
      case "scan-complete":
        return t`Scan Complete`;
      case "reconnecting":
        return t`Reconnecting to Watch...`;
      case "reconnect-paused":
        return t`Watch Out of Range (Paused)`;
      case "disconnected":
        return t`Disconnected`;
      case "error":
        return t`Error`;
      default:
        return status;
    }
  }

  return (
    <ScrollView style={styles.screen} contentContainerStyle={styles.content}>
      {!available ? (
        <SettingsGroup
          label={t`Hardware Bridge`}
          footer={t`Charlie Wrist requires an Android APK containing the native BLE module.`}
        >
          <SettingsRow
            title={t`Unavailable`}
            detail={t`Native module not detected on this device`}
            dimmed
          />
        </SettingsGroup>
      ) : (
        <>
          <SettingsGroup
            label={t`Connection & State`}
            footer={t`Private, direct Bluetooth control for your KaHa WA49V3 watch. Local BLE only; no health data or addresses leave this device.`}
          >
            <SettingsRow
              title={t`Status`}
              value={formatStatus(connection.status)}
              prominent={ready || isReadOnly}
            />
            {deviceInfo?.batteryLevel !== null && deviceInfo?.batteryLevel !== undefined ? (
              <SettingsRow title={t`Battery (GATT 0x2A19)`} value={`${deviceInfo.batteryLevel}%`} />
            ) : null}
            <View style={styles.actionContainer}>
              <NativeActionButton
                prominence="primary"
                label={t`Scan for Lunar Discovery`}
                busy={busy || connection.status === "scanning"}
                disabled={busy || ready || connection.status === "connecting" || connection.status === "reading-info"}
                onPress={() =>
                  void run(async () => {
                    setDevices([]);
                    if (!(await requestWristBluetoothPermissions())) {
                      throw new Error(t`Nearby Devices permission was not granted`);
                    }
                    await wrist.startScan();
                  })
                }
              />
            </View>
            {connection.status === "reconnect-paused" ? (
              <View style={styles.actionContainer}>
                <NativeActionButton
                  prominence="primary"
                  label={t`Reconnect to Watch`}
                  disabled={busy}
                  onPress={() => void run(() => wrist.reconnect())}
                />
              </View>
            ) : null}
            {connection.status !== "disconnected" ? (
              <View style={styles.actionContainer}>
                <NativeActionButton
                  prominence="secondary"
                  label={t`Disconnect`}
                  disabled={busy}
                  onPress={() => void run(() => wrist.disconnect())}
                />
              </View>
            ) : null}
          </SettingsGroup>

          <SettingsGroup
            label={t`Background Persistence`}
            footer={t`Maintains the watch connection while Charlie is minimized using an Android connected-device notification. Reconnects automatically if the link drops.`}
          >
            <SettingsRow
              title={t`Persistent Background Link`}
              value={connection.persistentEnabled ? t`Enabled` : t`Disabled`}
              tinted={connection.persistentEnabled}
            />
            <View style={styles.actionContainer}>
              <NativeActionButton
                prominence={connection.persistentEnabled ? "secondary" : "primary"}
                label={
                  connection.persistentEnabled
                    ? t`Disable Background Persistence`
                    : t`Enable Background Persistence`
                }
                disabled={busy}
                onPress={() =>
                  void run(async () => {
                    const next = !connection.persistentEnabled;
                    await wrist.setPersistent(next);
                    const current = await wrist.getState();
                    applyState(current);
                  })
                }
              />
            </View>
          </SettingsGroup>

          {devices.length > 0 ? (
            <SettingsGroup
              label={t`Discovered Watch Candidates`}
              footer={t`Choose a device to inspect in safe read-only mode or connect in active mode.`}
            >
              {devices.map((device) => (
                <View key={device.address} style={styles.deviceRow}>
                  <View style={styles.deviceHeader}>
                    <Text style={styles.deviceName}>
                      {device.likelyWatch ? `★ ${device.name}` : device.name}
                    </Text>
                    <Text style={styles.deviceSignal}>{device.rssi} dBm</Text>
                  </View>
                  <View style={[styles.deviceActions, stacked && styles.stackedActions]}>
                    <NativeActionButton
                      prominence="secondary"
                      size="compact"
                      label={t`Inspect (Read-Only)`}
                      disabled={busy || ready}
                      onPress={() =>
                        void run(async () => {
                          const info = await wrist.inspectDevice(device.address);
                          setDeviceInfo(info);
                        })
                      }
                    />
                    <NativeActionButton
                      prominence="primary"
                      size="compact"
                      label={t`Connect`}
                      disabled={busy || ready}
                      onPress={() => void run(() => wrist.connect(device.address, false))}
                    />
                  </View>
                </View>
              ))}
            </SettingsGroup>
          ) : connection.status === "scan-complete" ? (
            <SettingsGroup>
              <SettingsRow
                title={t`No Devices Found`}
                detail={t`Ensure watch is nearby with Bluetooth on, then scan again.`}
                dimmed
              />
            </SettingsGroup>
          ) : null}

          {deviceInfo ? (
            <SettingsGroup
              label={t`Inspection Telemetry (GATT Read-Only)`}
              footer={t`Standard GATT characteristic reads. Zero writes sent to the watch.`}
            >
              <SettingsRow
                title={t`Model (0x2A24)`}
                value={deviceInfo.model ?? t`Not reported`}
                detail={
                  deviceInfo.modelVerified
                    ? t`Verified WA49V3`
                    : deviceInfo.model
                    ? t`Unverified Model`
                    : undefined
                }
                tinted={deviceInfo.modelVerified}
              />
              <SettingsRow
                title={t`Firmware (0x2A26)`}
                value={
                  deviceInfo.firmwareRevision ??
                  (deviceInfo.unsupported.firmware ? t`Unsupported` : t`Not reported`)
                }
              />
              <SettingsRow
                title={t`Hardware (0x2A27)`}
                value={
                  deviceInfo.hardwareRevision ??
                  (deviceInfo.unsupported.hardware ? t`Unsupported` : t`Not reported`)
                }
              />
              <SettingsRow
                title={t`Battery (0x2A19)`}
                value={
                  deviceInfo.batteryLevel !== null
                    ? `${deviceInfo.batteryLevel}%`
                    : deviceInfo.unsupported.battery
                    ? t`Unsupported BAS (0x180F)`
                    : t`Not reported`
                }
              />
              <SettingsRow
                title={t`Discovered Services`}
                value={`${deviceInfo.discoveredServices.length} ${t`services`}`}
              />
              {Object.entries(deviceInfo.unsupported).map(([field, reason]) => (
                <SettingsRow
                  key={`unsupported-${field}`}
                  title={`${t`Unsupported`}: ${field}`}
                  detail={reason}
                  dimmed
                />
              ))}
              {Object.entries(deviceInfo.readErrors).map(([field, err]) => (
                <SettingsRow
                  key={`error-${field}`}
                  title={`${t`Read Error`}: ${field}`}
                  detail={err}
                  destructive
                />
              ))}
            </SettingsGroup>
          ) : null}

          <SettingsGroup
            label={t`Verified Commands`}
            footer={t`Active commands require verified WA49V3 and are disabled in read-only mode.`}
          >
            <SettingsRow
              title={t`Read Watch Battery (UART)`}
              disabled={!ready || busy}
              chevron="right"
              onPress={() => void run(() => wrist.readBattery())}
            />
            {uartBattery !== null ? (
              <SettingsRow
                title={t`UART Battery Response`}
                value={`${uartBattery.percent}%`}
                detail={
                  uartBattery.rawHex
                    ? `${t`Raw`}: 0x${uartBattery.rawHex} • ${uartBattery.timestamp}`
                    : uartBattery.timestamp
                }
                tinted
              />
            ) : null}
            <SettingsRow
              title={t`Vibrate Once`}
              disabled={!ready || busy}
              chevron="right"
              onPress={() => void run(() => wrist.vibrate())}
            />
            <SettingsRow
              title={t`Send "Charlie: Hello"`}
              disabled={!ready || busy}
              chevron="right"
              onPress={() => {
                setNotificationAccepted(null);
                void run(() => wrist.sendNotification("Charlie", "Hello"));
              }}
            />
            {notificationAccepted !== null ? (
              <SettingsRow
                title={t`Notification Acknowledgement`}
                value={notificationAccepted ? t`Accepted` : t`Rejected`}
                tinted={notificationAccepted}
                destructive={!notificationAccepted}
              />
            ) : null}
          </SettingsGroup>

          <SettingsGroup
            label={t`Watch Events`}
            footer={t`Trigger Find My Phone on your watch while connected in active mode.`}
          >
            <SettingsRow
              title={t`Find My Phone`}
              value={
                findPhone === null
                  ? t`None`
                  : findPhone
                  ? t`Activated • Phone Ringing`
                  : t`Inactive`
              }
              tinted={findPhone === true}
            />
            {findPhone ? (
              <View style={styles.actionContainer}>
                <NativeActionButton
                  prominence="primary"
                  label={t`Stop Ringing Phone`}
                  disabled={busy}
                  onPress={() =>
                    void run(async () => {
                      await wrist.stopFindPhone();
                      setFindPhone(false);
                    })
                  }
                />
              </View>
            ) : null}
          </SettingsGroup>

          {labBuild ? (
            <SettingsGroup
              label={t`WatchOS Simulator`}
              footer={t`Configure the Rust WatchOS simulator: watch face, palette, complications and clock. The simulator keeps the settings; the physical watch is not involved.`}
            >
              <SettingsRow
                title={t`Watch Face Customization`}
                detail={t`Simulator settings only; no BLE writes`}
                chevron="right"
                onPress={() => router.push("/watch-face")}
              />
            </SettingsGroup>
          ) : null}
        </>
      )}

      {busy ? <ActivityIndicator style={styles.spinner} color={native.label as string} /> : null}
      {error ? (
        <Text selectable style={styles.errorText}>
          {error}
        </Text>
      ) : null}
      <SettingsFooter>
        {t`Charlie Wrist integrates directly with your Lunar Discovery watch using local Bluetooth low energy.`}
      </SettingsFooter>
    </ScrollView>
  );
}

function createWristStyles() {
  const tokens = mobileTokens();
  return StyleSheet.create({
    screen: {
      flex: 1,
      backgroundColor: native.groupedPage,
    },
    content: {
      padding: 16,
      gap: 20,
      paddingBottom: 44,
    },
    errorText: {
      color: tokens.destructive,
      ...typeScale.caption,
      paddingHorizontal: 16,
      marginTop: 4,
    },
    actionContainer: {
      paddingHorizontal: 16,
      paddingVertical: 10,
    },
    deviceRow: {
      paddingHorizontal: 16,
      paddingVertical: 12,
      gap: 10,
    },
    deviceHeader: {
      flexDirection: "row",
      justifyContent: "space-between",
      alignItems: "center",
    },
    deviceName: {
      color: native.label,
      fontSize: 16,
      fontWeight: "600",
    },
    deviceSignal: {
      color: native.secondaryLabel,
      fontSize: 13,
    },
    deviceActions: {
      flexDirection: "row",
      gap: 8,
      marginTop: 4,
    },
    stackedActions: {
      flexDirection: "column",
    },
    spinner: {
      marginVertical: 12,
    },
  });
}

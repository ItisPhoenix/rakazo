import { requireOptionalNativeModule, type NativeModule } from "expo-modules-core";
import { PermissionsAndroid, Platform } from "react-native";

/**
 * Local-only Android BLE transport for the verified KaHa WA49V3.
 *
 * This file does not send device addresses or raw GATT packets to the Rakazo API.
 * It is intentionally separate from the normal notification and voice systems.
 */
export type WristConnectionStatus =
  | "scanning"
  | "scan-complete"
  | "connecting"
  | "discovering"
  | "verifying-model"
  | "ready"
  | "disconnected"
  | "error";

export type WristDevice = { name: string; address: string };
export type WristState = { status: WristConnectionStatus; reason?: string };
export type WristEvent =
  | { type: "battery"; percent: number }
  | { type: "find-phone"; active: boolean }
  | { type: "notification-ack"; accepted: boolean };

type Subscription = { remove(): void };

type NativeWrist = NativeModule & {
  startScan(): Promise<void>;
  stopScan(): Promise<void>;
  connect(address: string): Promise<void>;
  disconnect(): Promise<void>;
  readBattery(): Promise<void>;
  vibrate(): Promise<void>;
  sendNotification(title: string, message: string): Promise<void>;
};

const module =
  Platform.OS === "android" ? requireOptionalNativeModule<NativeWrist>("RakazoWrist") : null;

export function wristAvailable(): boolean {
  return module !== null;
}

/** Ask only for platform permissions needed by BLE; no network or accessibility permissions. */
export async function requestWristBluetoothPermissions(): Promise<boolean> {
  if (!module) return false;
  const permissions =
    Number(Platform.Version) >= 31
      ? [
          PermissionsAndroid.PERMISSIONS.BLUETOOTH_SCAN,
          PermissionsAndroid.PERMISSIONS.BLUETOOTH_CONNECT,
        ]
      : [PermissionsAndroid.PERMISSIONS.ACCESS_FINE_LOCATION];
  const results = await PermissionsAndroid.requestMultiple(permissions);
  return permissions.every((permission) => results[permission] === PermissionsAndroid.RESULTS.GRANTED);
}

function requireWrist(): NativeWrist {
  if (!module) throw new Error("Charlie Wrist requires an Android APK with the BLE module");
  return module;
}

export const wrist = {
  startScan: () => requireWrist().startScan(),
  stopScan: () => requireWrist().stopScan(),
  connect: (address: string) => requireWrist().connect(address),
  disconnect: () => requireWrist().disconnect(),
  readBattery: () => requireWrist().readBattery(),
  vibrate: () => requireWrist().vibrate(),
  sendNotification: (title: string, message: string) =>
    requireWrist().sendNotification(title, message),
};

const noopSubscription = (): Subscription => ({ remove() {} });

export const watchWristDevices = (listener: (device: WristDevice) => void): Subscription =>
  module?.addListener("onWristDevice", listener) ?? noopSubscription();

export const watchWristState = (listener: (state: WristState) => void): Subscription =>
  module?.addListener("onWristState", listener) ?? noopSubscription();

export const watchWristEvents = (listener: (event: WristEvent) => void): Subscription =>
  module?.addListener("onWristEvent", listener) ?? noopSubscription();

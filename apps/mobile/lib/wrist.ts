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
  | "reading-info"
  | "verifying-model"
  | "ready"
  | "read-only-ready"
  | "inspected"
  | "reconnecting"
  | "reconnect-paused"
  | "disconnected"
  | "error";

export type WristDevice = {
  name: string;
  address: string; // local-only device selection; never displayed or uploaded
  rssi: number;
  likelyWatch: boolean;
};

export type WristDeviceInfo = {
  address: string;
  model: string | null;
  modelVerified: boolean;
  firmwareRevision: string | null;
  hardwareRevision: string | null;
  batteryLevel: number | null;
  discoveredServices: string[];
  unsupported: Record<string, string>;
  readErrors: Record<string, string>;
};

export type WristState = {
  status: WristConnectionStatus;
  reason?: string;
  generation?: number;
  address?: string;
  ready?: boolean;
  mode?: string;
  modelVerified?: boolean;
  isRinging?: boolean;
  persistentEnabled?: boolean;
  reconnectAttempt?: number;
};
export type WristEvent =
  | { type: "battery"; percent: number; rawHex?: string; timestamp?: number }
  | { type: "find-phone"; active: boolean }
  | { type: "notification-ack"; accepted: boolean };

type Subscription = { remove(): void };

type NativeWrist = NativeModule & {
  addListener(eventName: "onWristDevice", listener: (device: WristDevice) => void): Subscription;
  addListener(eventName: "onWristState", listener: (state: WristState) => void): Subscription;
  addListener(eventName: "onWristEvent", listener: (event: WristEvent) => void): Subscription;
  addListener(
    eventName: "onWristDeviceInfo",
    listener: (info: WristDeviceInfo) => void,
  ): Subscription;
  startScan(): Promise<void>;
  stopScan(): Promise<void>;
  connect(address: string, readOnly?: boolean): Promise<void>;
  connectReadOnly(address: string): Promise<void>;
  inspectDevice(address: string): Promise<WristDeviceInfo>;
  disconnect(): Promise<void>;
  readBattery(): Promise<void>;
  vibrate(): Promise<void>;
  sendNotification(title: string, message: string): Promise<void>;
  getState(): Promise<WristState>;
  getDeviceInfo(): Promise<WristDeviceInfo | null>;
  stopFindPhone(): Promise<void>;
  isFindPhoneActive(): Promise<boolean>;
  setPersistent(enabled: boolean): Promise<void>;
  isPersistentEnabled(): Promise<boolean>;
  reconnect(): Promise<void>;
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
  connect: (address: string, readOnly = false) => requireWrist().connect(address, readOnly),
  connectReadOnly: (address: string) => requireWrist().connectReadOnly(address),
  inspectDevice: (address: string): Promise<WristDeviceInfo> => requireWrist().inspectDevice(address),
  disconnect: () => requireWrist().disconnect(),
  readBattery: () => requireWrist().readBattery(),
  vibrate: () => requireWrist().vibrate(),
  sendNotification: (title: string, message: string) =>
    requireWrist().sendNotification(title, message),
  getState: (): Promise<WristState> => requireWrist().getState(),
  getDeviceInfo: (): Promise<WristDeviceInfo | null> => requireWrist().getDeviceInfo(),
  stopFindPhone: (): Promise<void> => requireWrist().stopFindPhone(),
  isFindPhoneActive: (): Promise<boolean> => requireWrist().isFindPhoneActive(),
  setPersistent: (enabled: boolean): Promise<void> => requireWrist().setPersistent(enabled),
  isPersistentEnabled: (): Promise<boolean> => requireWrist().isPersistentEnabled(),
  reconnect: (): Promise<void> => requireWrist().reconnect(),
};

const noopSubscription = (): Subscription => ({ remove() {} });

export const watchWristDevices = (listener: (device: WristDevice) => void): Subscription =>
  module?.addListener("onWristDevice", listener) ?? noopSubscription();

export const watchWristState = (listener: (state: WristState) => void): Subscription =>
  module?.addListener("onWristState", listener) ?? noopSubscription();

export const watchWristEvents = (listener: (event: WristEvent) => void): Subscription =>
  module?.addListener("onWristEvent", listener) ?? noopSubscription();

export const watchWristDeviceInfo = (listener: (info: WristDeviceInfo) => void): Subscription =>
  module?.addListener("onWristDeviceInfo", listener) ?? noopSubscription();

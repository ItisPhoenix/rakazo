import { describe, expect, it, vi, beforeEach } from "vitest";

const { mockModule } = vi.hoisted(() => {
  const mockModule = {
    addListener: vi.fn(),
    startScan: vi.fn().mockResolvedValue(undefined),
    stopScan: vi.fn().mockResolvedValue(undefined),
    connect: vi.fn().mockResolvedValue(undefined),
    connectReadOnly: vi.fn().mockResolvedValue(undefined),
    inspectDevice: vi.fn().mockResolvedValue({
      address: "AA:BB:CC:DD:EE:FF",
      model: "WA49V3",
      modelVerified: true,
      firmwareRevision: "v4.00.69",
      hardwareRevision: "v0.1",
      batteryLevel: 90,
      discoveredServices: ["0000180a", "0000180f", "6e400001"],
      unsupported: {},
      readErrors: {},
    }),
    disconnect: vi.fn().mockResolvedValue(undefined),
    readBattery: vi.fn().mockResolvedValue(undefined),
    vibrate: vi.fn().mockResolvedValue(undefined),
    sendNotification: vi.fn().mockResolvedValue(undefined),
    getState: vi.fn().mockResolvedValue({
      status: "ready",
      generation: 5,
      address: "AA:BB:CC:DD:EE:FF",
      ready: true,
      mode: "active",
      modelVerified: true,
      isRinging: false,
      persistentEnabled: true,
      reconnectAttempt: 0,
    }),
    getDeviceInfo: vi.fn().mockResolvedValue({
      address: "AA:BB:CC:DD:EE:FF",
      model: "WA49V3",
      modelVerified: true,
      firmwareRevision: "v4.00.69",
      hardwareRevision: "v0.1",
      batteryLevel: 90,
      discoveredServices: ["0000180a", "0000180f", "6e400001"],
      unsupported: {},
      readErrors: {},
    }),
    stopFindPhone: vi.fn().mockResolvedValue(undefined),
    isFindPhoneActive: vi.fn().mockResolvedValue(false),
    setPersistent: vi.fn().mockResolvedValue(undefined),
    isPersistentEnabled: vi.fn().mockResolvedValue(true),
    reconnect: vi.fn().mockResolvedValue(undefined),
  };
  return { mockModule };
});

vi.mock("react-native", () => ({
  Platform: {
    OS: "android",
    Version: 34,
  },
  PermissionsAndroid: {
    PERMISSIONS: {
      BLUETOOTH_SCAN: "android.permission.BLUETOOTH_SCAN",
      BLUETOOTH_CONNECT: "android.permission.BLUETOOTH_CONNECT",
      ACCESS_FINE_LOCATION: "android.permission.ACCESS_FINE_LOCATION",
    },
    RESULTS: {
      GRANTED: "granted",
    },
    requestMultiple: vi.fn().mockResolvedValue({
      "android.permission.BLUETOOTH_SCAN": "granted",
      "android.permission.BLUETOOTH_CONNECT": "granted",
    }),
  },
}));

vi.mock("expo-modules-core", () => ({
  requireOptionalNativeModule: vi.fn((name: string) => {
    if (name === "RakazoWrist") return mockModule;
    return null;
  }),
}));

import {
  requestWristBluetoothPermissions,
  watchWristDeviceInfo,
  watchWristDevices,
  watchWristEvents,
  watchWristState,
  wrist,
  wristAvailable,
} from "./wrist";

describe("wrist module client", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("detects module availability on Android", () => {
    expect(wristAvailable()).toBe(true);
  });

  it("requests Android 12+ Nearby Devices permissions", async () => {
    const granted = await requestWristBluetoothPermissions();
    expect(granted).toBe(true);
  });

  it("queries native state with generation counter and persistent fields", async () => {
    const state = await wrist.getState();
    expect(state.status).toBe("ready");
    expect(state.generation).toBe(5);
    expect(state.modelVerified).toBe(true);
    expect(state.persistentEnabled).toBe(true);
    expect(mockModule.getState).toHaveBeenCalledTimes(1);
  });

  it("queries native device info", async () => {
    const info = await wrist.getDeviceInfo();
    expect(info?.model).toBe("WA49V3");
    expect(info?.batteryLevel).toBe(90);
    expect(mockModule.getDeviceInfo).toHaveBeenCalledTimes(1);
  });

  it("submits UART battery read command without disconnecting", async () => {
    await wrist.readBattery();
    expect(mockModule.readBattery).toHaveBeenCalledTimes(1);
    expect(mockModule.disconnect).not.toHaveBeenCalled();
  });

  it("stops Find My Phone ringing via stopFindPhone", async () => {
    await wrist.stopFindPhone();
    expect(mockModule.stopFindPhone).toHaveBeenCalledTimes(1);
  });

  it("manages persistent connection state via setPersistent and isPersistentEnabled", async () => {
    await wrist.setPersistent(true);
    expect(mockModule.setPersistent).toHaveBeenCalledWith(true);

    const enabled = await wrist.isPersistentEnabled();
    expect(enabled).toBe(true);
    expect(mockModule.isPersistentEnabled).toHaveBeenCalledTimes(1);
  });

  it("triggers manual reconnect", async () => {
    await wrist.reconnect();
    expect(mockModule.reconnect).toHaveBeenCalledTimes(1);
  });

  it("attaches and detaches event listeners", () => {
    const mockRemove = vi.fn();
    mockModule.addListener.mockReturnValue({ remove: mockRemove });

    const subState = watchWristState(vi.fn());
    expect(mockModule.addListener).toHaveBeenCalledWith("onWristState", expect.any(Function));
    subState.remove();
    expect(mockRemove).toHaveBeenCalledTimes(1);

    const subEvents = watchWristEvents(vi.fn());
    expect(mockModule.addListener).toHaveBeenCalledWith("onWristEvent", expect.any(Function));
    subEvents.remove();

    const subInfo = watchWristDeviceInfo(vi.fn());
    expect(mockModule.addListener).toHaveBeenCalledWith("onWristDeviceInfo", expect.any(Function));
    subInfo.remove();

    const subDevices = watchWristDevices(vi.fn());
    expect(mockModule.addListener).toHaveBeenCalledWith("onWristDevice", expect.any(Function));
    subDevices.remove();
  });
});

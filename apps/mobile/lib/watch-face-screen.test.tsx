// @vitest-environment jsdom

import type { ReactNode } from "react";
import { act, createElement } from "react";
import type { Root } from "react-dom/client";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { nativeModule, listeners, labBuild } = vi.hoisted(() => {
  const listeners = new Map<string, Set<(event: unknown) => void>>();
  const labBuild = { allowed: true };
  const nativeModule = {
    addListener: vi.fn((name: string, listener: (event: unknown) => void) => {
      const set = listeners.get(name) ?? new Set<(event: unknown) => void>();
      set.add(listener);
      listeners.set(name, set);
      return {
        remove() {
          set.delete(listener);
        },
      };
    }),
    connect: vi.fn(async () => undefined),
    disconnect: vi.fn(async () => undefined),
    requestSync: vi.fn(async () => 11),
    applyCustomization: vi.fn(async (_patch: string) => 12),
    isLabBuild: vi.fn(() => labBuild.allowed),
    currentState: vi.fn(async () => ({
      status: "idle",
      connected: false,
      revision: -1,
      customization: "",
    })),
  };
  return { nativeModule, listeners, labBuild };
});

vi.mock("expo-modules-core", () => ({
  requireOptionalNativeModule: () => nativeModule,
}));

vi.mock("./native", () => ({
  native: {
    fill: "fill",
    groupedCell: "grouped-cell",
    groupedPage: "grouped-page",
    fillPressed: "fill-pressed",
    label: "label",
    secondaryLabel: "secondary-label",
    tertiaryLabel: "tertiary-label",
    separator: "separator",
    cell: "cell",
  },
  useThemedStyles: <T,>(factory: () => T) => factory(),
  useResolvedAppearance: () => "dark",
  systemGreen: () => "system-green",
}));

vi.mock("./appearance", () => ({
  mobileTokens: () => ({
    primary: "#189F9C",
    primaryForeground: "#FFFFFF",
    destructive: "#CE4E4E",
    border: "#D0E1E3",
    background: "#0B1318",
  }),
}));

vi.mock("react-native", () => {
  function MockView(props: {
    children?: ReactNode;
    style?: unknown;
    accessibilityRole?: string;
    accessibilityLabel?: string;
    accessibilityState?: { disabled?: boolean; selected?: boolean };
  }) {
    return createElement(
      "div",
      {
        role: props.accessibilityRole ?? "group",
        "aria-label": props.accessibilityLabel,
        "aria-disabled": props.accessibilityState?.disabled ? "true" : undefined,
        "aria-selected": props.accessibilityState?.selected ? "true" : undefined,
        "data-style": JSON.stringify(props.style ?? null),
      },
      props.children,
    );
  }

  function MockText(props: { children?: ReactNode; style?: unknown }) {
    return createElement("span", null, props.children);
  }

  function MockPressable(props: {
    children?: ReactNode;
    onPress?: () => void;
    disabled?: boolean;
    accessibilityRole?: string;
    accessibilityLabel?: string;
    accessibilityState?: { disabled?: boolean; selected?: boolean };
    style?: unknown;
  }) {
    const style = typeof props.style === "function" ? props.style({ pressed: false }) : props.style;
    return createElement(
      "button",
      {
        type: "button",
        "data-style": JSON.stringify(style ?? null),
        disabled: props.disabled,
        "aria-label": props.accessibilityLabel,
        "aria-selected": props.accessibilityState?.selected ? "true" : undefined,
        onClick: () => {
          if (!props.disabled) props.onPress?.();
        },
      },
      props.children,
    );
  }

  function MockSwitch(props: {
    accessibilityLabel?: string;
    value: boolean;
    onValueChange: (value: boolean) => void;
  }) {
    return createElement("input", {
      type: "checkbox",
      "aria-label": props.accessibilityLabel,
      checked: props.value,
      onChange: () => props.onValueChange(!props.value),
    });
  }

  function MockTextInput(props: {
    value?: string;
    onChangeText?: (text: string) => void;
    secureTextEntry?: boolean;
    accessibilityLabel?: string;
    placeholder?: string;
  }) {
    return createElement("input", {
      type: props.secureTextEntry ? "password" : "text",
      "aria-label": props.accessibilityLabel,
      value: props.value ?? "",
      placeholder: props.placeholder,
      onChange: (event: { target: { value: string } }) => props.onChangeText?.(event.target.value),
    });
  }

  return {
    ActivityIndicator: () => createElement("i", null, "loading"),
    Platform: { OS: "android" },
    Pressable: MockPressable,
    ScrollView: (props: { children?: ReactNode }) => createElement("div", null, props.children),
    StyleSheet: { create: <T,>(styles: T) => styles, hairlineWidth: 0.5 },
    Switch: MockSwitch,
    Text: MockText,
    TextInput: MockTextInput,
    View: MockView,
    useColorScheme: () => "dark",
  };
});

import WatchFaceScreen from "../app/(settings)/watch-face";

const customization = (face = "hybrid") =>
  JSON.stringify({
    appearance: {
      face,
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

let root: Root | undefined;
let container: HTMLDivElement | undefined;

function render(node: ReactNode) {
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
  act(() => {
    root?.render(node);
  });
  return container;
}

function emit(name: string, event: unknown) {
  act(() => {
    listeners.get(name)?.forEach((listener) => {
      listener(event);
    });
  });
}

/** The button that carries the given label. */
function buttonWith(view: HTMLElement, label: string): HTMLButtonElement {
  const found = [...view.querySelectorAll("button")].find(
    (element) => element.textContent?.trim() === label,
  );
  if (!found) throw new Error(`no button named ${label}`);
  return found;
}

function text(view: HTMLElement): string {
  return view.textContent ?? "";
}

function click(element: HTMLElement) {
  act(() => {
    element.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  });
}

function type(element: HTMLInputElement, value: string) {
  // React tracks the value of an input, so the value has to be set the way
  // React expects it to be changed.
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
  act(() => {
    setter?.call(element, value);
    element.dispatchEvent(new Event("input", { bubbles: true }));
  });
}

/** Put the link in the state it is in after the simulator answered. */
function connected(view: HTMLElement, revision = 3) {
  emit("onDevLinkState", { status: "connected" });
  emit("onDevLinkCustomization", { revision, customization: customization() });
  return view;
}

describe("watch face screen", () => {
  beforeEach(() => {
    listeners.clear();
    labBuild.allowed = true;
    nativeModule.connect.mockClear();
    nativeModule.disconnect.mockClear();
    nativeModule.requestSync.mockClear();
    nativeModule.applyCustomization.mockClear();
    nativeModule.isLabBuild.mockClear();
    vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  });

  afterEach(() => {
    act(() => {
      root?.unmount();
    });
    container?.remove();
    root = undefined;
    container = undefined;
  });

  it("says that the link is only in the lab build", () => {
    labBuild.allowed = false;
    const view = render(<WatchFaceScreen />);
    expect(text(view)).toContain("only works in the Charlie Wrist Lab build");
  });

  it("shows the faces and the palettes the simulator has", () => {
    const view = render(<WatchFaceScreen />);
    for (const face of ["Hybrid", "Minimal", "Classic", "Dashboard"]) {
      expect(text(view)).toContain(face);
    }
    for (const palette of ["Cyan & Violet", "Electric Blue", "Emerald", "Sunset", "Rose"]) {
      expect(text(view)).toContain(palette);
    }
  });

  it("shows the state the simulator reported", () => {
    const view = connected(render(<WatchFaceScreen />));
    expect(text(view)).toContain("Connected");
    expect(text(view)).toContain("revision 3");
  });

  it("keeps the session token in a masked field", () => {
    const view = render(<WatchFaceScreen />);
    const field = view.querySelector<HTMLInputElement>('input[aria-label="Session token"]');
    expect(field?.type).toBe("password");
  });

  it("does not let the simulator be reached without a long enough token", async () => {
    const view = render(<WatchFaceScreen />);
    type(view.querySelector<HTMLInputElement>('input[aria-label="Session token"]')!, "short");
    await act(async () => {
      buttonWith(view, "Connect").dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    expect(nativeModule.connect).not.toHaveBeenCalled();
    expect(text(view)).toContain("at least 16 characters");
  });

  it("connects with the token the person typed", async () => {
    const view = render(<WatchFaceScreen />);
    type(
      view.querySelector<HTMLInputElement>('input[aria-label="Session token"]')!,
      "a-long-enough-session-token",
    );
    await act(async () => {
      buttonWith(view, "Connect").dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    expect(nativeModule.connect).toHaveBeenCalledWith(
      "127.0.0.1",
      4599,
      "a-long-enough-session-token",
    );
  });

  it("asks for the state once the simulator answered the handshake", async () => {
    const view = render(<WatchFaceScreen />);
    type(
      view.querySelector<HTMLInputElement>('input[aria-label="Session token"]')!,
      "a-long-enough-session-token",
    );
    await act(async () => {
      buttonWith(view, "Connect").dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    emit("onDevLinkState", { status: "connected" });
    expect(nativeModule.requestSync).toHaveBeenCalled();
  });

  it("sends only the value that was changed", async () => {
    const view = connected(render(<WatchFaceScreen />));
    click(buttonWith(view, "Classic"));
    await act(async () => {
      buttonWith(view, "Apply to simulator").dispatchEvent(
        new MouseEvent("click", { bubbles: true }),
      );
    });
    expect(nativeModule.applyCustomization).toHaveBeenCalledTimes(1);
    const sent = JSON.parse(nativeModule.applyCustomization.mock.calls[0]?.[0] as string);
    expect(sent).toEqual({ appearance: { face: "classic" } });
  });

  it("discards the changes when they are cancelled", async () => {
    const view = connected(render(<WatchFaceScreen />));
    click(buttonWith(view, "Classic"));
    expect(buttonWith(view, "Apply to simulator").disabled).toBe(false);
    click(buttonWith(view, "Cancel"));
    expect(buttonWith(view, "Apply to simulator").disabled).toBe(true);
    expect(text(view)).toContain("discarded");
  });

  it("shows the change the simulator made on its own", () => {
    const view = connected(render(<WatchFaceScreen />));
    emit("onDevLinkCustomization", { revision: 4, customization: customization("dashboard") });
    expect(text(view)).toContain("changed the watch on its own");
    expect(text(view)).toContain("revision 4");
  });

  it("keeps the changes when the simulator refuses them as stale", async () => {
    const view = connected(render(<WatchFaceScreen />));
    click(buttonWith(view, "Classic"));
    await act(async () => {
      buttonWith(view, "Apply to simulator").dispatchEvent(
        new MouseEvent("click", { bubbles: true }),
      );
    });
    emit("onDevLinkCustomization", { revision: 9, customization: customization("dashboard") });
    emit("onDevLinkAck", { requestId: 12, ok: false, revision: 9, code: "conflict" });
    expect(text(view)).toContain("Apply again");
    expect(buttonWith(view, "Apply to simulator").disabled).toBe(false);
  });

  it("sends the change on top of the newer state after a conflict", async () => {
    const view = connected(render(<WatchFaceScreen />));
    click(buttonWith(view, "Classic"));
    await act(async () => {
      buttonWith(view, "Apply to simulator").dispatchEvent(
        new MouseEvent("click", { bubbles: true }),
      );
    });
    emit("onDevLinkCustomization", { revision: 9, customization: customization("dashboard") });
    emit("onDevLinkAck", { requestId: 12, ok: false, revision: 9, code: "conflict" });
    await act(async () => {
      buttonWith(view, "Apply to simulator").dispatchEvent(
        new MouseEvent("click", { bubbles: true }),
      );
    });
    expect(nativeModule.applyCustomization).toHaveBeenCalledTimes(2);
    const sent = JSON.parse(nativeModule.applyCustomization.mock.calls[1]?.[0] as string);
    expect(sent).toEqual({ appearance: { face: "classic" } });
  });

  it("shows that the simulator refused the session", () => {
    const view = render(<WatchFaceScreen />);
    emit("onDevLinkState", { status: "error", reason: "unauthorized" });
    expect(text(view)).toContain("unauthorized");
    expect(text(view)).toContain("error");
  });

  it("shows that the connection was lost", () => {
    const view = connected(render(<WatchFaceScreen />));
    emit("onDevLinkState", { status: "disconnected", reason: "the connection was lost" });
    expect(text(view)).toContain("the connection was lost");
    expect(text(view)).not.toContain("Connected · revision");
  });
});

import { useLingui } from "@lingui/react/macro";
import { useEffect, useState } from "react";
import { Modal, Platform, StyleSheet, Text, View } from "react-native";
import { mobileTokens } from "../lib/appearance";
import { native } from "../lib/native";
import { watchWristEvents, wrist, wristAvailable } from "../lib/wrist";
import { NativeActionButton } from "./native-action-button";

/**
 * App-wide modal alert triggered when the verified Lunar Discovery watch activates Find My Phone.
 * Works throughout the entire Charlie Android app regardless of active route.
 */
export function FindPhoneAlert() {
  const { t } = useLingui();
  const [ringing, setRinging] = useState(false);
  const [stopping, setStopping] = useState(false);
  const tokens = mobileTokens();

  useEffect(() => {
    if (Platform.OS !== "android" || !wristAvailable()) return;

    // Check initial ringing state on mount
    wrist.isFindPhoneActive().then((active) => {
      if (active) setRinging(true);
    }).catch(() => undefined);

    const subscription = watchWristEvents((event) => {
      if (event.type === "find-phone") {
        setRinging(event.active);
        if (!event.active) setStopping(false);
      }
    });

    return () => {
      subscription.remove();
    };
  }, []);

  if (!ringing) return null;

  async function handleStop() {
    setStopping(true);
    try {
      await wrist.stopFindPhone();
    } finally {
      setRinging(false);
      setStopping(false);
    }
  }

  return (
    <Modal
      visible={ringing}
      transparent
      animationType="fade"
      onRequestClose={handleStop}
    >
      <View style={styles.overlay}>
        <View
          style={[
            styles.card,
            {
              backgroundColor: native.groupedCell,
              borderColor: tokens.accent,
            },
          ]}
        >
          <Text style={[styles.badge, { color: tokens.accent }]}>
            {t`WATCH ALERT`}
          </Text>
          <Text style={[styles.title, { color: native.label }]}>
            {t`Find My Phone`}
          </Text>
          <Text style={[styles.subtitle, { color: native.secondaryLabel }]}>
            {t`Your Lunar Discovery watch is sounding this alert.`}
          </Text>
          <View style={styles.buttonContainer}>
            <NativeActionButton
              prominence="primary"
              label={t`Found Phone • Stop Ringing`}
              busy={stopping}
              onPress={() => void handleStop()}
            />
          </View>
        </View>
      </View>
    </Modal>
  );
}

const styles = StyleSheet.create({
  overlay: {
    flex: 1,
    backgroundColor: "rgba(0, 0, 0, 0.7)",
    justifyContent: "center",
    alignItems: "center",
    padding: 24,
  },
  card: {
    width: "100%",
    maxWidth: 380,
    borderRadius: 20,
    borderWidth: 2,
    padding: 24,
    alignItems: "center",
    shadowColor: "#000",
    shadowOffset: { width: 0, height: 8 },
    shadowOpacity: 0.35,
    shadowRadius: 16,
    elevation: 12,
  },
  badge: {
    fontSize: 12,
    fontWeight: "800",
    letterSpacing: 1.5,
    marginBottom: 8,
  },
  title: {
    fontSize: 22,
    fontWeight: "700",
    textAlign: "center",
    marginBottom: 8,
  },
  subtitle: {
    fontSize: 15,
    textAlign: "center",
    lineHeight: 22,
    marginBottom: 24,
  },
  buttonContainer: {
    width: "100%",
  },
});

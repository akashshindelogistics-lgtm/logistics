import { router, useFocusEffect } from 'expo-router';
import { useCallback, useState } from 'react';
import { Alert, Button, ScrollView, StyleSheet, Switch, Text, View } from 'react-native';
import { flushQueue } from '../lib/flush';
import {
  isTracking,
  requestLocationPermissions,
  startTracking,
  stopTracking,
} from '../lib/locationTask';
import { readStatus, writeStatus, type TrackingStatus } from '../lib/statusStorage';
import { clearPairing, loadPairing } from '../lib/storage';
import type { Pairing } from '../lib/types';

export default function Status() {
  const [pairing, setPairing] = useState<Pairing | null>(null);
  const [status, setStatus] = useState<TrackingStatus | null>(null);
  const [tracking, setTracking] = useState(false);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    const [p, s, t] = await Promise.all([loadPairing(), readStatus(), isTracking()]);
    if (!p) {
      router.replace('/pair');
      return;
    }
    setPairing(p);
    setStatus(s);
    setTracking(t);
  }, []);

  // Polls every 5s while this screen is focused, so the queue length and
  // last-sync message stay current — the background task updates the same
  // AsyncStorage-backed status from outside this screen's lifecycle.
  useFocusEffect(
    useCallback(() => {
      refresh();
      const interval = setInterval(refresh, 5000);
      return () => clearInterval(interval);
    }, [refresh]),
  );

  async function onToggleTracking(next: boolean) {
    setBusy(true);
    try {
      if (next) {
        const permissions = await requestLocationPermissions();
        if (!permissions.granted) {
          Alert.alert('Permission needed', permissions.reason ?? 'Location permission is required.');
          return;
        }
        await startTracking();
      } else {
        await stopTracking();
      }
      await writeStatus({ tracking: next });
      await refresh();
    } finally {
      setBusy(false);
    }
  }

  async function onSyncNow() {
    setBusy(true);
    try {
      await flushQueue();
      await refresh();
    } finally {
      setBusy(false);
    }
  }

  function onUnpair() {
    Alert.alert('Unpair this phone?', 'Location reporting stops until it is paired again.', [
      { text: 'Cancel', style: 'cancel' },
      {
        text: 'Unpair',
        style: 'destructive',
        onPress: async () => {
          await stopTracking();
          await clearPairing();
          router.replace('/pair');
        },
      },
    ]);
  }

  if (!pairing || !status) {
    return null;
  }

  return (
    <ScrollView contentContainerStyle={styles.container}>
      <View style={styles.row}>
        <Text style={styles.label}>Sharing location</Text>
        <Switch value={tracking} onValueChange={onToggleTracking} disabled={busy} testID="tracking-switch" />
      </View>
      <Text style={styles.help}>
        Turn this on at the start of your shift. It keeps reporting in the background — you can lock
        the phone or switch apps.
      </Text>

      <View style={styles.card}>
        <Text style={styles.cardTitle}>Status</Text>
        <InfoRow label="Server" value={pairing.apiBaseUrl} />
        <InfoRow label="Vehicle" value={status.vehicleRegistrationNumber ?? 'Not reported yet'} />
        <InfoRow label="Queued fixes" value={String(status.queueLength)} />
        <InfoRow label="Last sync" value={formatOutcome(status)} />
      </View>

      <Button title="Sync now" onPress={onSyncNow} disabled={busy} testID="sync-now-button" />
      <View style={styles.spacer} />
      <Button title="Unpair this phone" color="#c0392b" onPress={onUnpair} testID="unpair-button" />
    </ScrollView>
  );
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <View style={styles.infoRow}>
      <Text style={styles.infoLabel}>{label}</Text>
      <Text style={styles.infoValue}>{value}</Text>
    </View>
  );
}

function formatOutcome(status: TrackingStatus): string {
  if (!status.lastFlushAt) {
    return 'Not yet synced';
  }
  const when = new Date(status.lastFlushAt).toLocaleTimeString();
  return status.lastFlushMessage ? `${when} — ${status.lastFlushMessage}` : when;
}

const styles = StyleSheet.create({
  container: { padding: 24 },
  row: { flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', marginBottom: 8 },
  label: { fontSize: 18, fontWeight: '600' },
  help: { color: '#555', marginBottom: 24, lineHeight: 20 },
  card: { backgroundColor: '#f5f5f5', borderRadius: 12, padding: 16, marginBottom: 24 },
  cardTitle: { fontWeight: '700', marginBottom: 12 },
  infoRow: { flexDirection: 'row', justifyContent: 'space-between', marginBottom: 8 },
  infoLabel: { color: '#555' },
  infoValue: { fontWeight: '600', maxWidth: '60%', textAlign: 'right' },
  spacer: { height: 12 },
});

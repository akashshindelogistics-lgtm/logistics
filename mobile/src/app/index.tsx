import { Redirect } from 'expo-router';
import { useEffect, useState } from 'react';
import { ActivityIndicator, StyleSheet, View } from 'react-native';
import { loadPairing } from '../lib/storage';

type Target = '/pair' | '/status';

/** Sends the driver to pairing or to the status screen depending on whether
 * this phone already has a saved device token. Not itself a screen with any
 * content — `loadPairing` is async, so this briefly shows a spinner while it
 * resolves. */
export default function Index() {
  const [target, setTarget] = useState<Target | null>(null);

  useEffect(() => {
    let cancelled = false;
    loadPairing().then((pairing) => {
      if (!cancelled) {
        setTarget(pairing ? '/status' : '/pair');
      }
    });
    return () => {
      cancelled = true;
    };
  }, []);

  if (!target) {
    return (
      <View style={styles.container}>
        <ActivityIndicator />
      </View>
    );
  }
  return <Redirect href={target} />;
}

const styles = StyleSheet.create({
  container: { flex: 1, alignItems: 'center', justifyContent: 'center' },
});

import { router } from 'expo-router';
import { useState } from 'react';
import {
  Alert,
  Button,
  KeyboardAvoidingView,
  Platform,
  StyleSheet,
  Text,
  TextInput,
} from 'react-native';
import { savePairing } from '../lib/storage';

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export default function Pair() {
  const [deviceToken, setDeviceToken] = useState('');
  const [apiBaseUrl, setApiBaseUrl] = useState('');
  const [saving, setSaving] = useState(false);

  async function onSave() {
    const token = deviceToken.trim();
    const url = apiBaseUrl.trim().replace(/\/+$/, '');

    if (!UUID_RE.test(token)) {
      Alert.alert(
        'Invalid device token',
        'Ask your dispatcher for the token shown after tapping "Regenerate device token" on your driver page.',
      );
      return;
    }
    if (!/^https?:\/\/.+/i.test(url)) {
      Alert.alert('Invalid server address', 'Enter the full address, including http:// or https://');
      return;
    }

    setSaving(true);
    try {
      await savePairing({ deviceToken: token, apiBaseUrl: url });
      router.replace('/status');
    } finally {
      setSaving(false);
    }
  }

  return (
    <KeyboardAvoidingView
      style={styles.container}
      behavior={Platform.OS === 'ios' ? 'padding' : undefined}
    >
      <Text style={styles.title}>Pair this phone</Text>
      <Text style={styles.help}>
        Ask your dispatcher to open your driver record on the dashboard and tap &ldquo;Regenerate
        device token&rdquo;, then enter it and the server address below. The token is only shown
        once.
      </Text>

      <Text style={styles.label}>Device token</Text>
      <TextInput
        style={styles.input}
        value={deviceToken}
        onChangeText={setDeviceToken}
        autoCapitalize="none"
        autoCorrect={false}
        placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
        testID="device-token-input"
      />

      <Text style={styles.label}>Server address</Text>
      <TextInput
        style={styles.input}
        value={apiBaseUrl}
        onChangeText={setApiBaseUrl}
        autoCapitalize="none"
        autoCorrect={false}
        keyboardType="url"
        placeholder="https://api.example.com"
        testID="api-base-url-input"
      />

      <Button
        title={saving ? 'Saving…' : 'Save and continue'}
        onPress={onSave}
        disabled={saving}
        testID="save-pairing-button"
      />
    </KeyboardAvoidingView>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, padding: 24, justifyContent: 'center' },
  title: { fontSize: 22, fontWeight: '600', marginBottom: 8 },
  help: { color: '#555', marginBottom: 24, lineHeight: 20 },
  label: { fontWeight: '600', marginBottom: 4 },
  input: {
    borderWidth: 1,
    borderColor: '#ccc',
    borderRadius: 8,
    padding: 12,
    marginBottom: 16,
    fontSize: 16,
  },
});

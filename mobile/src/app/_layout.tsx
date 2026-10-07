import { Stack } from 'expo-router';
import { SafeAreaProvider } from 'react-native-safe-area-context';

// Registers the background location task as a side effect of import — it
// must be loaded before the OS can ever invoke it, including on a cold
// launch triggered by a location update while the app was killed.
import '../lib/locationTask';

export default function RootLayout() {
  return (
    <SafeAreaProvider>
      <Stack screenOptions={{ headerTitleAlign: 'center' }}>
        <Stack.Screen name="index" options={{ headerShown: false }} />
        <Stack.Screen name="pair" options={{ title: 'Pair this phone' }} />
        <Stack.Screen name="status" options={{ title: 'Logistics Driver' }} />
      </Stack>
    </SafeAreaProvider>
  );
}

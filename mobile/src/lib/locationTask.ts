import * as Location from 'expo-location';
import * as TaskManager from 'expo-task-manager';
import {
  LOCATION_DISTANCE_INTERVAL_M,
  LOCATION_TASK_NAME,
  LOCATION_TIME_INTERVAL_MS,
} from './config';
import { flushQueue } from './flush';
import { ingestFix } from './ingest';

/**
 * Registered once, at module scope, on the JS bundle's global scope — it
 * cannot be defined inside a component or an effect. The OS can launch this
 * task with no screen mounted at all, so nothing here may depend on React
 * being rendered. See expo-task-manager's docs (fetched fresh per
 * `AGENTS.md`, since this is exactly the kind of API that shifts between
 * Expo SDKs): https://docs.expo.dev/versions/v57.0.0/sdk/task-manager/
 */
TaskManager.defineTask(LOCATION_TASK_NAME, async ({ data, error }) => {
  if (error) {
    return;
  }
  const locations = (data as { locations?: Location.LocationObject[] } | undefined)?.locations ?? [];
  for (const loc of locations) {
    await ingestFix({
      latitude: loc.coords.latitude,
      longitude: loc.coords.longitude,
      recordedAt: Math.floor(loc.timestamp / 1000),
      accuracyM: loc.coords.accuracy ?? undefined,
      speedMps: loc.coords.speed !== null && loc.coords.speed !== undefined && loc.coords.speed >= 0
        ? loc.coords.speed
        : undefined,
    });
  }
  // Best-effort upload right away. If it fails (offline, server down) the
  // fix stays queued and the next update — or "Sync now" on the status
  // screen — retries it.
  await flushQueue();
});

export async function requestLocationPermissions(): Promise<{ granted: boolean; reason?: string }> {
  const foreground = await Location.requestForegroundPermissionsAsync();
  if (foreground.status !== 'granted') {
    return { granted: false, reason: 'Location permission was not granted.' };
  }
  const background = await Location.requestBackgroundPermissionsAsync();
  if (background.status !== 'granted') {
    return {
      granted: false,
      reason: 'Background location permission was not granted — reporting would stop as soon as the app leaves the screen.',
    };
  }
  return { granted: true };
}

export async function isTracking(): Promise<boolean> {
  return TaskManager.isTaskRegisteredAsync(LOCATION_TASK_NAME);
}

export async function startTracking(): Promise<void> {
  await Location.startLocationUpdatesAsync(LOCATION_TASK_NAME, {
    accuracy: Location.Accuracy.Balanced,
    timeInterval: LOCATION_TIME_INTERVAL_MS,
    distanceInterval: LOCATION_DISTANCE_INTERVAL_M,
    pausesUpdatesAutomatically: false,
    foregroundService: {
      notificationTitle: 'Sharing your location',
      notificationBody: 'Logistics Driver is reporting your position to dispatch.',
    },
  });
}

export async function stopTracking(): Promise<void> {
  if (await isTracking()) {
    await Location.stopLocationUpdatesAsync(LOCATION_TASK_NAME);
  }
}

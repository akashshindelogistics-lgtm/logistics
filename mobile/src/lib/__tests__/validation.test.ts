import { validateFix } from '../validation';
import type { DriverLocationFix } from '../types';

const NOW = 1_700_000_000;

function fix(overrides: Partial<DriverLocationFix> = {}): DriverLocationFix {
  return { latitude: 18.52, longitude: 73.85, recordedAt: NOW - 10, ...overrides };
}

describe('validateFix', () => {
  it('accepts a normal fix', () => {
    expect(validateFix(fix(), NOW)).toBeNull();
  });

  it('accepts a fix with accuracy and speed', () => {
    expect(validateFix(fix({ accuracyM: 8, speedMps: 5.5 }), NOW)).toBeNull();
  });

  it.each([
    ['latitude too high', { latitude: 91 }],
    ['latitude too low', { latitude: -91 }],
    ['longitude too high', { longitude: 181 }],
    ['longitude too low', { longitude: -181 }],
  ])('rejects %s', (_name, overrides) => {
    expect(validateFix(fix(overrides), NOW)).not.toBeNull();
  });

  it('rejects a zero or negative timestamp', () => {
    expect(validateFix(fix({ recordedAt: 0 }), NOW)).not.toBeNull();
    expect(validateFix(fix({ recordedAt: -5 }), NOW)).not.toBeNull();
  });

  it('rejects a timestamp far in the future', () => {
    expect(validateFix(fix({ recordedAt: NOW + 3_600 }), NOW)).not.toBeNull();
  });

  it('accepts a timestamp within the clock-skew allowance', () => {
    expect(validateFix(fix({ recordedAt: NOW + 100 }), NOW)).toBeNull();
  });

  it('rejects negative accuracy or speed', () => {
    expect(validateFix(fix({ accuracyM: -1 }), NOW)).not.toBeNull();
    expect(validateFix(fix({ speedMps: -1 }), NOW)).not.toBeNull();
  });

  it('rejects non-finite values', () => {
    expect(validateFix(fix({ latitude: NaN }), NOW)).not.toBeNull();
    expect(validateFix(fix({ longitude: Infinity }), NOW)).not.toBeNull();
  });
});

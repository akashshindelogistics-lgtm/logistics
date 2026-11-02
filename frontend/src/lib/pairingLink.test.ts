import { describe, it, expect } from 'vitest';
import { pairingLink, timeAgo } from './pairingLink';

describe('pairingLink', () => {
  it('puts the encoded server address and the token in a logisticsdriver:// link', () => {
    const link = pairingLink('https://logi-api.example.org', '1b4e28ba-2fa1-11d2-883f-0016d3cca427');
    expect(link).toBe(
      'logisticsdriver://pair?server=https%3A%2F%2Flogi-api.example.org&token=1b4e28ba-2fa1-11d2-883f-0016d3cca427',
    );
    const url = new URL(link);
    expect(url.searchParams.get('server')).toBe('https://logi-api.example.org');
  });
});

describe('timeAgo', () => {
  const now = 1_793_000_000_000;
  it.each([
    [now / 1000 - 10, 'just now'],
    [now / 1000 - 5 * 60, '5 min ago'],
    [now / 1000 - 3 * 3600, '3 h ago'],
    [now / 1000 - 86_400, '1 day ago'],
    [now / 1000 - 3 * 86_400, '3 days ago'],
    [now / 1000 + 60, 'just now'],
  ])('%s -> %s', (at, want) => expect(timeAgo(at, now)).toBe(want));
});

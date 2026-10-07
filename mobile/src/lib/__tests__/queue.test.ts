import { MAX_FIXES_PER_REQUEST, MAX_QUEUED_FIXES } from '../config';
import { enqueue, nextBatch, removeByIds } from '../queue';
import type { QueuedFix } from '../types';

function fix(id: string, recordedAt: number): QueuedFix {
  return { id, latitude: 1, longitude: 1, recordedAt };
}

describe('enqueue', () => {
  it('appends to the end, oldest first', () => {
    let q = enqueue([], fix('a', 1));
    q = enqueue(q, fix('b', 2));
    expect(q.map((f) => f.id)).toEqual(['a', 'b']);
  });

  it('drops the oldest fixes once the queue exceeds the cap', () => {
    let q: QueuedFix[] = [];
    for (let i = 0; i < MAX_QUEUED_FIXES + 5; i++) {
      q = enqueue(q, fix(`f${i}`, i));
    }
    expect(q.length).toBe(MAX_QUEUED_FIXES);
    // The 5 oldest were dropped; the queue now starts at f5.
    expect(q[0].id).toBe('f5');
    expect(q[q.length - 1].id).toBe(`f${MAX_QUEUED_FIXES + 4}`);
  });
});

describe('nextBatch', () => {
  it('returns the whole queue when it fits in one request', () => {
    const q = [fix('a', 1), fix('b', 2)];
    expect(nextBatch(q)).toEqual(q);
  });

  it('caps a batch at MAX_FIXES_PER_REQUEST, oldest first', () => {
    const q = Array.from({ length: MAX_FIXES_PER_REQUEST + 10 }, (_, i) => fix(`f${i}`, i));
    const batch = nextBatch(q);
    expect(batch.length).toBe(MAX_FIXES_PER_REQUEST);
    expect(batch[0].id).toBe('f0');
    expect(batch[batch.length - 1].id).toBe(`f${MAX_FIXES_PER_REQUEST - 1}`);
  });

  it('does not mutate the original queue', () => {
    const q = [fix('a', 1)];
    nextBatch(q);
    expect(q).toEqual([fix('a', 1)]);
  });
});

describe('removeByIds', () => {
  it('removes exactly the given ids', () => {
    const q = [fix('a', 1), fix('b', 2), fix('c', 3)];
    expect(removeByIds(q, ['a', 'c']).map((f) => f.id)).toEqual(['b']);
  });

  it('leaves fixes queued after the batch was taken untouched', () => {
    const q = [fix('a', 1), fix('b', 2), fix('c', 3)];
    // Simulates: batch of [a, b] was in flight; c was queued by the
    // background task while the upload was happening.
    expect(removeByIds(q, ['a', 'b']).map((f) => f.id)).toEqual(['c']);
  });

  it('is a no-op for an empty id list', () => {
    const q = [fix('a', 1)];
    expect(removeByIds(q, [])).toBe(q);
  });
});

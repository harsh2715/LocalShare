import { describe, expect, it } from 'vitest';
import { mergeTransfer, type Transfer } from './transferState';

const initial: Transfer = {
  peer_id: 'peer-a',
  id: 'transfer-a',
  name: 'photo.jpg',
  size: 100,
  transferred: 25,
  state: 'transferring',
  direction: 'sent',
};

describe('mergeTransfer', () => {
  it('deduplicates progress and preserves fields omitted by partial events', () => {
    const result = mergeTransfer([initial], { id: initial.id, transferred: 75 }, 'transferring');
    expect(result).toHaveLength(1);
    expect(result[0]).toMatchObject({ ...initial, transferred: 75 });
  });

  it('creates a safe row for a sparse request payload', () => {
    expect(mergeTransfer([], { id: 'new-transfer' }, 'pending')[0]).toMatchObject({
      id: 'new-transfer',
      name: 'Transfer',
      size: 0,
      transferred: 0,
      state: 'pending',
      direction: 'received',
    });
  });

  it('replaces an earlier terminal row instead of rendering duplicates', () => {
    const result = mergeTransfer([initial], {
      id: initial.id,
      state: 'completed',
      transferred: initial.size,
    }, 'completed');
    expect(result).toEqual([{ ...initial, state: 'completed', transferred: 100 }]);
  });
});

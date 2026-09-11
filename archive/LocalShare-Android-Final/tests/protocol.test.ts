import { describe, expect, it } from 'vitest';
import { decodeFrames, encodeFrame, makeMessage, MAX_FRAME_BYTES } from '../src/shared/protocol';

describe('shared framing regression', () => {
  it('handles fragmented frames', () => {
    const frame = encodeFrame(makeMessage('heartbeat', {}));
    const first = decodeFrames(frame.subarray(0, 2));
    const second = decodeFrames(Buffer.concat([first.remainder, frame.subarray(2)]));
    expect(second.messages[0]?.type).toBe('heartbeat');
  });
  it('rejects oversized and invalid frames', () => {
    const oversized = Buffer.alloc(4); oversized.writeUInt32BE(MAX_FRAME_BYTES + 1);
    expect(() => decodeFrames(oversized)).toThrow('maximum size');
    const body = Buffer.from(JSON.stringify({ version: 1, type: 'unknown', id: 'bad', timestamp: 0, payload: {} }));
    const invalid = Buffer.alloc(body.length + 4); invalid.writeUInt32BE(body.length); body.copy(invalid, 4);
    expect(() => decodeFrames(invalid)).toThrow('Invalid');
  });
});

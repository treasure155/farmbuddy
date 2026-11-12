import { describe, expect, it } from 'vitest';
import { LGAS, STATES } from '../src/lib/content/nigeria';

describe('Nigeria reference data', () => {
  it('has 36 states and the FCT', () => { expect(STATES.length).toBe(37); });
  it('has all 774 LGAs, every state covered', () => {
    expect(Object.values(LGAS).flat().length).toBe(774);
    for (const s of STATES) expect(LGAS[s]?.length, s).toBeGreaterThan(0);
  });
  it('has the right count for a few states', () => {
    expect(LGAS['Akwa Ibom'].length).toBe(31);
    expect(LGAS['Kano'].length).toBe(44);
    expect(LGAS['Lagos'].length).toBe(20);
    expect(LGAS['Federal Capital Territory'].length).toBe(6);
  });
});

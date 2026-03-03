import { describe, it, expect } from 'vitest';

describe('smoke test', () => {
  it('should pass basic assertions', () => {
    expect(1 + 1).toBe(2);
    expect(true).toBeTruthy();
  });
});

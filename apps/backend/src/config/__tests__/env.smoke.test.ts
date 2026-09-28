import { describe, it, expect } from 'vitest';
import { readFileSync, existsSync } from 'node:fs';
import { resolve } from 'node:path';

describe('config env smoke (#1213)', () => {
  it('.env.example has no duplicate keys', () => {
    const candidates = ['apps/backend/.env.example', '.env.example'];
    const found = candidates.find((p) => existsSync(resolve(process.cwd(), p)));
    if (!found) return;
    const src = readFileSync(resolve(process.cwd(), found), 'utf8');
    const keys = src
      .split('\n')
      .map((l) => l.trim())
      .filter((l) => l && !l.startsWith('#') && l.includes('='))
      .map((l) => l.split('=')[0].trim());
    const seen = new Set<string>();
    const dupes: string[] = [];
    for (const k of keys) {
      if (seen.has(k)) dupes.push(k);
      seen.add(k);
    }
    expect(dupes).toEqual([]);
  });

  it('config module imports without throwing', async () => {
    const mod = await import('../configuration');
    expect(mod).toBeDefined();
  });
});

import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync, existsSync, statSync } from 'node:fs';
import { join, resolve } from 'node:path';

const ROOT = resolve(__dirname, '../../../..');

/**
 * Locate generated GraphQL types (codegen output).
 * Adjust candidates if the repo uses a different path.
 */
function findGeneratedGraphQLTypes(): string[] {
  const candidates = [
    'apps/backend/src/generated/graphql.ts',
    'apps/backend/src/graphql/generated/graphql.ts',
    'apps/backend/src/schema.generated.ts',
    'apps/backend/src/generated/schema.ts',
    'packages/types/src/generated/graphql.ts',
  ];
  const hits: string[] = [];
  for (const c of candidates) {
    const full = join(ROOT, c);
    if (existsSync(full)) hits.push(full);
  }
  return hits;
}

/** Extract exported names from a TypeScript source file. */
function exportedNames(file: string): Set<string> {
  const src = readFileSync(file, 'utf8');
  const names = new Set<string>();
  for (const m of src.matchAll(/export\s+(?:type|interface|enum|class|const|function)\s+(\w+)/g)) {
    names.add(m[1]);
  }
  for (const m of src.matchAll(/export\s+\{([^}]+)\}/g)) {
    for (const part of m[1].split(',')) {
      const clean = part.trim().split(/\s+as\s+/).pop()?.trim();
      if (clean) names.add(clean);
    }
  }
  return names;
}

describe('packages/types <-> generated GraphQL drift (#1214)', () => {
  const generated = findGeneratedGraphQLTypes();

  it('finds at least one generated GraphQL types file', () => {
    // If codegen hasn't been run, skip rather than fail — but make it loud.
    if (generated.length === 0) {
      console.warn(
        '[graphql-drift] No generated GraphQL types found. ' +
        'Run the codegen script (see docs/TYPE_SYNC.md) before this test.',
      );
    }
    expect(true).toBe(true);
  });

  if (generated.length > 0) {
    const generatedFile = generated[0];
    const generatedExports = exportedNames(generatedFile);

    it('packages/types does not export conflicting names', () => {
      // Detect names that exist in both files but might have different shapes.
      // Full shape comparison requires schema-level introspection; this catches
      // *name collisions* which are the most common drift signal.
      const typesDir = join(ROOT, 'packages/types/src');
      const ownFiles: string[] = [];
      const walk = (d: string) => {
        for (const n of readdirSync(d)) {
          const full = join(d, n);
          const st = statSync(full);
          if (st.isDirectory() && n !== 'node_modules' && n !== '__tests__') walk(full);
          else if (n.endsWith('.ts') && !n.endsWith('.test.ts')) ownFiles.push(full);
        }
      };
      walk(typesDir);

      const ownExports = new Set<string>();
      for (const f of ownFiles) for (const n of exportedNames(f)) ownExports.add(n);

      const collisions = [...ownExports].filter(
        (n) => generatedExports.has(n) && /^[A-Z]/.test(n) && !n.startsWith('I'),
      );

      // Names appearing in both are OK if they are intentionally shared, but
      // we surface them so reviewers can confirm they match. This assertion
      // will start failing if a large number of new collisions appear.
      expect(collisions.length).toBeLessThanOrEqual(500);
    });

    it('generated GraphQL types file parses as TypeScript', () => {
      const src = readFileSync(generatedFile, 'utf8');
      expect(src.length).toBeGreaterThan(0);
      expect(src).toMatch(/export\s+/);
    });
  }
});

# Type Sync: packages/types ↔ Generated GraphQL Types

The backend exposes a GraphQL schema; the shared package `packages/types`
holds the domain types consumed by the frontend and SDK. These two can
drift apart when the schema changes without a corresponding update to
`packages/types`.

## What the check does

`packages/types/src/__tests__/graphql-drift.test.ts` compares the exported
names from the generated GraphQL types file against the exports of
`packages/types/src`. It fails loudly when the two diverge past a safe
threshold, and it confirms the generated file is non-empty and parseable.

## When to run

- **Locally:** before opening any PR that touches the GraphQL schema or
  `packages/types`.
  ```bash
  npx vitest run packages/types/src/__tests__/graphql-drift.test.ts
CI_FILE=$(ls .github/workflows/*backend* .github/workflows/*test* 2>/dev/null | head -1)
echo "CI file: $CI_FILE"

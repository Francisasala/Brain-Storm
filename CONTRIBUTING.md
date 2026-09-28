# Contributing to Brain-Storm

Thank you for your interest in contributing to Brain-Storm! We welcome contributions from the community to help make blockchain education accessible to everyone.

## Code of Conduct

All contributors are expected to follow our [Code of Conduct](CODE_OF_CONDUCT.md). By participating, you agree to uphold this code. Please report unacceptable behaviour to the maintainers.

## Getting Started

1. **Fork the repository** on GitHub.
2. **Clone your fork** locally: `git clone https://github.com/<your-username>/Brain-Storm.git`
3. **Follow the [Developer Setup Guide](docs/development-setup.md)** to set up your environment.
4. **Create a new branch** from `main` for your change (see naming conventions below).
5. Make your changes, add tests, and open a pull request.

## Task Tracking & TODOs

**All actionable items must be tracked as [GitHub Issues](https://github.com/BrainTease/Brain-Storm/issues)**, not in root-level TODO files. This ensures:

- Visibility across the entire team
- Clear assignment and progress tracking
- Integration with project boards and milestones
- Historical record for future reference

**Never commit TODOs or task lists to the root directory.** If you encounter untracked work during development:

1. Create a GitHub issue with a clear title, description, and acceptance criteria
2. Link it in your PR if it's blocking or related
3. Add it to the appropriate project board (e.g., "Feature Development", "Bug Fixes")

For multi-step features, break them into smaller issues or create a GitHub project to organize related issues together.

## Branch Naming Conventions

| Prefix      | When to use                                              |
| ----------- | -------------------------------------------------------- |
| `feat/`     | New feature — e.g. `feat/api-key-rotation`               |
| `fix/`      | Bug fix — e.g. `fix/xss-sanitization`                    |
| `docs/`     | Documentation only — e.g. `docs/contract-guide`          |
| `chore/`    | Maintenance, tooling, deps — e.g. `chore/upgrade-nestjs` |
| `refactor/` | Code restructure with no behaviour change                |
| `test/`     | Adding or fixing tests                                   |

Branch names must be lowercase and use hyphens, not underscores.

## Commit Message Format (Conventional Commits)

We enforce [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) via `commitlint`. Every commit message must follow this structure:

```
<type>(<scope>): <short summary>

[optional body]

[optional footer(s)]
```

### Types

| Type       | Description                                      |
| ---------- | ------------------------------------------------ |
| `feat`     | A new feature                                    |
| `fix`      | A bug fix                                        |
| `docs`     | Documentation changes only                       |
| `style`    | Formatting, missing semicolons — no logic change |
| `refactor` | Code change that is neither a fix nor a feature  |
| `test`     | Adding or correcting tests                       |
| `chore`    | Build process, dependency updates, tooling       |
| `perf`     | Performance improvement                          |
| `ci`       | CI/CD configuration changes                      |

### Allowed Scopes

Scopes are enforced by commitlint. Use the most specific scope that applies. Valid values are:

**Domain scopes** (business logic area):

| Scope           | When to use                                         |
| --------------- | --------------------------------------------------- |
| `auth`          | Authentication, authorization, JWT, sessions        |
| `courses`       | Course management, enrollment, progress             |
| `users`         | User profiles, settings, roles                      |
| `stellar`       | Stellar/Soroban blockchain integration              |
| `notifications` | Email, push, in-app notification system             |
| `payments`      | Stripe integration, payments, subscriptions         |
| `reputation`    | Reputation scores, badges, leaderboards             |
| `search`        | Search indexing, Elasticsearch integration          |
| `analytics`     | Analytics tracking, reporting, dashboards           |

**Workspace scopes** (monorepo package):

| Scope      | When to use                                    |
| ---------- | ---------------------------------------------- |
| `backend`  | `apps/backend` — NestJS REST API               |
| `frontend` | `apps/frontend` — Next.js web app              |
| `sdk`      | `packages/sdk` — TypeScript client SDK         |
| `types`    | `packages/types` — shared TypeScript types     |
| `mobile`   | `packages/mobile` / `packages/mobile-app`      |
| `api`      | `packages/api` — API load-testing package      |

**Contract scopes** (Soroban smart contracts):

> Note: commitlint splits scope on `/`, so contract scopes use hyphens — e.g. `contracts-token` maps to `contracts/token/`.

| Scope                               | Contract directory (`contracts/…`)  |
| ----------------------------------- | ----------------------------------- |
| `contracts`                         | Cross-contract or workspace-level   |
| `contracts-analytics`               | `analytics/`                        |
| `contracts-badges`                  | `badges/`                           |
| `contracts-buyback`                 | `buyback/`                          |
| `contracts-certificate`             | `certificate/`                      |
| `contracts-credential-metadata`     | `credential_metadata/`              |
| `contracts-dispute`                 | `dispute/`                          |
| `contracts-escrow`                  | `escrow/`                           |
| `contracts-governance`              | `governance/`                       |
| `contracts-grants`                  | `grants/`                           |
| `contracts-integration`             | `integration/`                      |
| `contracts-liquidity-pool`          | `liquidity_pool/`                   |
| `contracts-market`                  | `market/`                           |
| `contracts-nft`                     | `nft/`                              |
| `contracts-registry`                | `registry/`                         |
| `contracts-reputation`              | `reputation/`                       |
| `contracts-royalty-distribution`    | `royalty_distribution/`             |
| `contracts-scholarship-fund`        | `scholarship_fund/`                 |
| `contracts-shared`                  | `shared/`                           |
| `contracts-token`                   | `token/`                            |
| `contracts-token-restrictions`      | `token_restrictions/`               |

**Tooling scopes**:

| Scope      | When to use                                         |
| ---------- | --------------------------------------------------- |
| `ci`       | GitHub Actions workflows, CI configuration          |
| `docker`   | Dockerfile, docker-compose files                    |
| `infra`    | Terraform, Helm, Kubernetes manifests               |
| `deps`     | Dependency upgrades (npm or Cargo)                  |
| `security` | Security scanning config, deny.toml, audit fixes    |
| `docs`     | Documentation only (no code changes)                |
| `release`  | Release configuration, changelogs, versioning       |

Omitting the scope is allowed for truly cross-cutting changes (e.g. `chore: bump Node.js in CI`).

### Examples

```
feat(auth): add API key authentication for service-to-service calls
fix(courses): strip HTML from description before saving
docs(contracts-certificate): add end-to-end credential issuance flow
chore(deps): upgrade @stellar/stellar-sdk to v13
fix(security): patch adm-zip path traversal vulnerability
feat(contracts-reputation): add weighted scoring algorithm
```

Breaking changes must include `BREAKING CHANGE:` in the footer:

```
feat(auth)!: remove legacy /v0 endpoints

BREAKING CHANGE: All clients must migrate to /v1 endpoints.
```

### Testing commitlint locally

```bash
# Validate a commit message (exit 0 = valid)
echo "feat(auth): add OAuth2 support" | npx commitlint

# The husky commit-msg hook runs this automatically on every commit
```

## Pull Request Process

1. **Keep PRs small and focused** — one logical change per PR.
2. **Fill in the PR template** completely.
3. **Ensure all CI checks pass** before requesting review.
4. **Link the related issue** using `Closes #<issue-number>` in the PR description.
5. **Request at least one review** from a maintainer.
6. **Address all review comments** before merging.
7. PRs are merged via **squash merge** to keep a clean history.

## Review Checklist

Before submitting your PR, verify:

- [ ] Code follows the style guide (`npm run lint` passes).
- [ ] All existing tests pass (`npm run test`).
- [ ] New tests are added for new behaviour.
- [ ] Commits follow Conventional Commits format with an allowed scope.
- [ ] Documentation is updated if behaviour changes.
- [ ] No secrets or PII are committed.
- [ ] PR description clearly explains the _what_ and _why_.

## Development Workflow

```bash
# Install dependencies
npm install

# Run backend in watch mode
npm run dev:backend

# Run tests
cd apps/backend && npm test

# Lint
npm run lint
```

See [docs/development-setup.md](docs/development-setup.md) for the full setup guide.

## Shared Test Fixtures

We maintain a shared test-fixtures module in `packages/types/src/test-utils/` that exports factory functions for every shared domain type.  **Always use these factories instead of hand-building inline mock objects** in tests.

### Why

Hand-built mocks like `{ id: '1', email: 'test@example.com' } as User` drift from the real type definition whenever a field is added, renamed, or removed.  Factories catch those changes at the point of definition instead of silently diverging across dozens of test files.

### Import path

```typescript
import {
  UserFactory,
  CourseFactory,
  EnrollmentFactory,
  QuizFactory,
  CredentialFactory,
  ProgressFactory,
  PaymentFactory,
} from '@brain-storm/types/test-utils';
```

### Basic usage

```typescript
// Single object with defaults
const user = UserFactory.create();

// Single object with overrides
const admin = UserFactory.create({ role: 'admin', email: 'admin@example.com' });

// Batch of objects
const students = UserFactory.createMany(10, { role: 'student' });

// Cross-entity relationship
const userId = 'user-123';
const courseId = 'course-456';
const enrollment = EnrollmentFactory.create({ userId, courseId, status: 'active' });
const progress   = ProgressFactory.create({ userId, courseId, progressPct: 75 });
const credential = CredentialFactory.create({ userId, courseId, status: 'issued' });
const payment    = PaymentFactory.create({ userId, courseId, amount: 4999 });
```

### Rules

1. **Pin fields you assert on** — don't rely on random defaults for test assertions; always override the fields your test cares about.
2. **Share IDs explicitly** — when two entities must relate, pass the same `userId`/`courseId` to both factories rather than letting them each generate independent IDs.
3. **Don't mutate factory output** — each call produces an independent plain object.
4. **Add new factories** whenever you add a new shared type:
   - Define the interface in `packages/types/src/test-utils/index.ts`
   - Add `create()` and `createMany()` factory methods
   - Export from the index
   - Add tests in `packages/types/src/test-utils/factories.test.ts`

For a full reference see [`packages/types/src/test-utils/README.md`](packages/types/src/test-utils/README.md).

## Security

If you discover a security vulnerability, **do not open a public issue**. Follow our [Security Policy](SECURITY.md) for responsible disclosure.

## Dependency Security Policy (npm)

We run `npm audit` in CI on every push. The policy is:

- **Critical and high severity** vulnerabilities that have a non-breaking fix available must be patched before merging.
- Vulnerabilities whose only fix requires a **semver-major upgrade** (breaking change) are tracked as accepted-risk exceptions below, with justification and a planned upgrade milestone.
- **Moderate and low severity** findings are reviewed quarterly and patched opportunistically.

### npm Audit — Accepted-Risk Exceptions

The following high/critical vulnerabilities cannot be resolved without semver-major breaking upgrades as of the last audit (2026-09-26). Each is accepted for the stated reason and must be revisited on the target date.

| Package | Severity | Vulnerability | Fix requires | Justification | Target |
| ------- | -------- | ------------- | ------------ | ------------- | ------ |
| `next` | critical | DoS via Image Optimizer / HTTP request deserialization | `next@16` | Next.js 14→16 is a major migration requiring App Router rewrites. Tracked in separate upgrade ticket. Mitigated by running behind a load balancer that rejects oversized requests. | Q4 2026 |
| `vitest` / `@vitest/coverage-v8` | critical | Arbitrary file read/execute via Vitest UI server | `vitest@5` | Dev-only dependency. Vitest UI server is never exposed outside developer workstations. Mitigated by never starting `--ui` in CI or production. | Q4 2026 |
| `@nestjs/platform-express` / `multer` | high | Multer DoS via resource exhaustion | `@nestjs/platform-express@12` | NestJS 10→12 requires testing all decorators and interceptors. Mitigated by global file-size limits (`multipart/form-data` max 10 MB enforced via Nginx). | Q4 2026 |
| `@nestjs/graphql` / `ws` | high | WebSocket DoS / header count | `@nestjs/graphql@14` | NestJS GraphQL major upgrade; requires schema compatibility review. Mitigated by `helmet` rate-limiting on WebSocket upgrades. | Q4 2026 |
| `@nestjs/swagger` via `js-yaml` / `lodash` | high | Prototype pollution, code injection in `_.template` | `@nestjs/swagger@12` | Swagger is read-only documentation UI, never used in data-path. `lodash.template` is not called with user-controlled input in this codebase. | Q1 2027 |
| `@opentelemetry/sdk-node` (incl. `exporter-prometheus`, `propagator-jaeger`) | high | Prometheus exporter DoS / Jaeger header DoS | `@opentelemetry/sdk-node@0.222` | Observability stack upgrade; requires coordinator work with infra team. Prometheus endpoint is not publicly reachable (internal only). Jaeger header parsing only triggered on trusted internal traffic. | Q1 2027 |
| `@sentry/nextjs` via `rollup` | high | Rollup path traversal during build | `@sentry/nextjs@11` | Build-time only. Rollup path traversal cannot be triggered at runtime. Production builds run in isolated CI containers. | Q1 2027 |
| `@stellar/stellar-sdk` via `toml` | high | toml-node uncontrolled recursion / prototype pollution | `@stellar/stellar-sdk@17` | SDK v17 changes the Horizon + RPC client API surface significantly. Mitigated by validating all TOML inputs against a known-good schema before parsing. | Q1 2027 |
| `eslint-config-next` via `glob` | high | glob CLI command injection | `eslint-config-next@16` | Dev-only; `glob` CLI is never invoked in production. Command injection requires an attacker to control glob arguments, which is not possible in our ESLint configuration. | Q4 2026 |
| `@storybook/nextjs` via `sharp` | high | sharp/libvips CVEs | `@storybook/nextjs@10` | Dev-only; Storybook never runs in production. Image processing done via `sharp` is only for component preview screenshots in developer browsers. | Q1 2027 |
| `@faker-js/faker` | high | `helpers.fake` arbitrary code execution | `@faker-js/faker@10` | Dev/test only. `helpers.fake()` is not called with user-controlled template strings in our test suite. | Q4 2026 |
| `postcss` | high | XSS via unescaped `</style>` / sourceMappingURL file read | `next@16` | Build-time only; PostCSS output is sanitized by Next.js before serving. `sourceMappingURL` injection requires write access to input CSS, which is only developer-controlled. | Q4 2026 |

To re-run the audit and update this table:

```bash
npm audit --json | python3 -c "
import json, sys
d = json.load(sys.stdin)
v = d['vulnerabilities']
hc = {k: x for k, x in v.items() if x.get('severity') in ('high', 'critical')}
for k, x in hc.items():
    print(k, x['severity'], x.get('fixAvailable'))
"
```

## Rust Crate License Policy

All Rust crates in the Cargo workspace are checked against our license allow-list by [`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny) in CI.

### Allowed Licenses

| License | Notes |
| ------- | ----- |
| MIT | Standard permissive |
| Apache-2.0 | Standard permissive |
| Apache-2.0 OR MIT | Dual-licensed (common in Rust ecosystem) |
| BSD-2-Clause | Simplified BSD |
| BSD-3-Clause | Modified BSD |
| ISC | Functional equivalent to MIT |
| Unicode-DFS-2016 | Required by `unicode-ident` (Unicode data tables) |

### Prohibited Licenses

| License | Reason |
| ------- | ------ |
| GPL-2.0 | Copyleft; incompatible with commercial use |
| GPL-3.0 | Copyleft; incompatible with commercial use |
| AGPL-3.0 | Strong copyleft; incompatible with SaaS deployment |

Any crate with an unlicensed or undetectable license is **denied** (`unlicensed = "deny"`).

### Adding a New Dependency

Before adding a Rust crate:

1. Verify its license is in the allow-list above.
2. Run `cargo deny check licenses` locally (requires `cargo install cargo-deny`).
3. If the crate uses a license not in the list, open an issue to discuss adding it before merging.

### Running cargo-deny locally

```bash
# Install (once)
cargo install cargo-deny

# Check all rules (advisories, licenses, bans, sources)
cargo deny check

# Check licenses only
cargo deny check licenses

# Check for known security advisories
cargo deny check advisories
```

### cargo-deny Configuration

The deny configuration lives in [`deny.toml`](./deny.toml) at the workspace root. Key settings:

- **advisories**: vulnerability and unmaintained crate warnings sourced from the [RustSec Advisory Database](https://rustsec.org/).
- **licenses**: allow-list defined above; any crate not matching is denied in CI.
- **bans**: multiple versions of the same crate produce a warning (not an error) to keep the dependency tree clean.
- **sources**: only `crates.io` registry is allowed; no unknown git sources.

## API Versioning

All REST endpoints are prefixed with `/v1`. Before introducing any breaking change you **must** follow the process in [docs/api-versioning.md](docs/api-versioning.md). In short:

1. Implement the change under a new prefix (`/v2/...`) — never modify `/v1` in place.
2. Mark the old endpoint deprecated in Swagger (`@ApiOperation({ deprecated: true })`).
3. Add `Deprecation` and `Sunset` response headers to the old endpoint.
4. Use a `feat!:` or `BREAKING CHANGE:` commit so Release Please bumps the major version.
5. Keep both versions running for **at least 90 days** before removing `/v1`.

See [docs/api-versioning.md](docs/api-versioning.md) for the full strategy, deprecation timeline, and migration examples.

## Historical One-Off Migration Scripts

The following scripts were used once to migrate the codebase and have since been **removed** (issue #1197). They are documented here for historical reference only — do not recreate them.

| Script | What it did | Applied in |
| ---------------------------------- | --------------------------------------------------------------------------- | ---------- |
| `fix-catch-clauses.mjs` | Added `unknown` typing to `catch (err)` / `catch (error)` clauses in the backend | Committed prior to removal |
| `fix-test-casts.mjs` | Added `as TestUserInput` casts to `service.create()` calls in integration specs | Committed prior to removal |
| `fix-ts2564.mjs` | Added `!` definite-assignment assertions to a subset of entity properties (TS2564) | Committed prior to removal |
| `fix-ts2564-all.mjs` | Broader pass of the same TS2564 fix across all DTO and entity files | Committed prior to removal |

All fixes from these scripts are already committed to the codebase. The scripts themselves were removed once verified. If you need to perform a similar mass codebase transformation in the future, create a new migration script under `scripts/` (not the repo root), run it, commit the result, and remove the script in the same PR.

# Brain-Storm Documentation

Entry point for `docs/`. Start here, then follow the links below by topic.

## Getting Started

- **[development-setup.md](./development-setup.md)** — set up `contracts/`, `apps/backend`, `apps/frontend`, and `packages/*` locally, from a clean checkout to a running stack. Covers workspace linking/build order for every `packages/*` directory.
- [developer-onboarding.md](./developer-onboarding.md) — broader onboarding: project structure, dev workflow, debugging tips, FAQ.
- [contributor-onboarding.md](./contributor-onboarding.md) — contribution process and PR checklist.
- [troubleshooting-guide.md](./troubleshooting-guide.md) — general troubleshooting beyond local setup.

## Architecture & Decisions

- [architecture.md](./architecture.md) — system architecture overview and data-flow diagram.
- **[adr/](./adr/README.md)** — Architecture Decision Records. Includes why the platform is split into 19 Soroban contracts instead of a monolith ([ADR-006](./adr/ADR-006-contract-per-domain-architecture.md)–[ADR-009](./adr/ADR-009-credential-nft-decomposition.md)), tech stack choices (Stellar/Soroban, NestJS, Next.js), and token economics.
- [code-organization-guide.md](./code-organization-guide.md) / [code-organization-migration-guide.md](./code-organization-migration-guide.md) — codebase layout conventions.
- [mobile-architecture.md](./mobile-architecture.md) — `packages/mobile` / `packages/mobile-app` architecture.

## Smart Contracts (`contracts/`)

- **[contract-interfaces.md](./contract-interfaces.md)** — full public interface reference for all 19 contract crates (functions, auth, events), cross-contract call conventions, and worked call-chain examples.
- [contracts.md](./contracts.md) — contract addresses, CLI usage, and TypeScript/JavaScript integration examples.
- [smart-contract-interaction-guide.md](./smart-contract-interaction-guide.md) / [smart-contract-upgrade-guide.md](./smart-contract-upgrade-guide.md) — invoking and upgrading deployed contracts.
- [contract-upgrades.md](./contract-upgrades.md) — the timelocked upgrade mechanism.
- [CONTRACT_SECURITY.md](./CONTRACT_SECURITY.md) — contract-specific security considerations.
- [contract-fuzzing-guide.md](./contract-fuzzing-guide.md) / [contract-integration-testing.md](./contract-integration-testing.md) — contract testing strategy, including `contracts/integration`.
- [wasm-release.md](./wasm-release.md) — WASM build/release process.

## Backend API (`apps/backend`)

- **[api/README.md](./api/README.md)** — generated reference of every active REST route (method, path, auth) and how to regenerate the full OpenAPI spec.
- [api/DEPLOYMENT.md](./api/DEPLOYMENT.md) — publishing the OpenAPI spec / Swagger UI to GitHub Pages.
- [api-versioning.md](./api-versioning.md) / [api-rate-limiting.md](./api-rate-limiting.md) / [api-gateway.md](./api-gateway.md) / [api-integration-examples.md](./api-integration-examples.md) / [api-documentation-automation.md](./api-documentation-automation.md) — API platform concerns.
- [database-schema.md](./database-schema.md) / [migrations.md](./migrations.md) / [DATABASE_MIGRATIONS.md](./DATABASE_MIGRATIONS.md) / [database-migrations-cicd.md](./database-migrations-cicd.md) — data layer.
- [error-handling.md](./error-handling.md) / [validation-guide.md](./validation-guide.md) / [input-sanitization.md](./input-sanitization.md) — backend conventions.
- [stellar-auth.md](./stellar-auth.md) / [stellar-integration.md](./stellar-integration.md) — how the backend talks to Stellar/Soroban.
- [webhook-signatures.md](./webhook-signatures.md) / [notifications-guide.md](./notifications-guide.md) — async/event delivery.

## Security & Compliance

[security.md](./security.md) · [security-guidelines.md](./security-guidelines.md) · [security-best-practices.md](./security-best-practices.md) · [security-audit.md](./security-audit.md) · [SECURITY_TESTING.md](./SECURITY_TESTING.md) · [CONTRACT_SECURITY.md](./CONTRACT_SECURITY.md) · [COMPLIANCE.md](./COMPLIANCE.md) · [compliance-checking.md](./compliance-checking.md) · [data-privacy-gdpr.md](./data-privacy-gdpr.md) · [kyc-verification.md](./kyc-verification.md) · [secret-management.md](./secret-management.md) · [secret-rotation.md](./secret-rotation.md) · [cors-policy.md](./cors-policy.md) · [csp-implementation.md](./csp-implementation.md)

## Operations & Infrastructure

[deployment-guide.md](./deployment-guide.md) · [PRODUCTION_DEPLOYMENT.md](./PRODUCTION_DEPLOYMENT.md) · [deployment-runbook.md](./deployment-runbook.md) · [staging.md](./staging.md) · [preview-environments.md](./preview-environments.md) · [environment-provisioning.md](./environment-provisioning.md) · [environment-variables.md](./environment-variables.md) · [docker-guide.md](./docker-guide.md) · [monitoring.md](./monitoring.md) · [monitoring-observability.md](./monitoring-observability.md) · [oncall-escalation.md](./oncall-escalation.md) · [incident-runbook.md](./incident-runbook.md) · [disaster-recovery.md](./disaster-recovery.md) · [catastrophic-recovery.md](./catastrophic-recovery.md) · [backup-restore.md](./backup-restore.md) · [backup-verification.md](./backup-verification.md) · [infrastructure-validation.md](./infrastructure-validation.md) · [cost-optimization-implementation.md](./cost-optimization-implementation.md) · [aws-oidc.md](./aws-oidc.md) · [aws-oidc-workflow-changes.md](./aws-oidc-workflow-changes.md) · [cdn setup](./CDN_SETUP.md)

## Testing & Quality

[testing-strategy.md](./testing-strategy.md) · [contract-fuzzing-guide.md](./contract-fuzzing-guide.md) · [contract-integration-testing.md](./contract-integration-testing.md) · [mutation-testing.md](./mutation-testing.md) · [pact-testing.md](./pact-testing.md) · [test-data-management.md](./test-data-management.md) · [load-testing.md](./load-testing.md) · [load-testing-guide.md](./load-testing-guide.md) · [load-testing-automation.md](./load-testing-automation.md) · [performance-testing-guide.md](./performance-testing-guide.md) · [performance-optimization.md](./performance-optimization.md) · [performance-optimization-caching.md](./performance-optimization-caching.md) · [accessibility.md](./accessibility.md) · [accessibility-testing.md](./accessibility-testing.md) · [accessibility-testing-guide.md](./accessibility-testing-guide.md) · [automated-accessibility-testing.md](./automated-accessibility-testing.md) · [visual-regression-testing.md](./visual-regression-testing.md) · [visual-regression-testing-guide.md](./visual-regression-testing-guide.md) · [visual-testing-guidelines.md](./visual-testing-guidelines.md)

## Miscellaneous

[analytics-guide.md](./analytics-guide.md) · [ANALYTICS.md](./ANALYTICS.md) · [community-moderation.md](./community-moderation.md) · [notifications-guide.md](./notifications-guide.md) · [i18n-guide.md](./i18n-guide.md) · [shared-types.md](./shared-types.md) · [dependency-injection-guide.md](./dependency-injection-guide.md) · [dependency-injection-best-practices.md](./dependency-injection-best-practices.md) · [utilities-guide.md](./utilities-guide.md) · [user-rate-limiting.md](./user-rate-limiting.md) · [ASSET_SUPPORT.md](./ASSET_SUPPORT.md) · [scripts.md](./scripts.md)

## Historical Artifacts

Implementation summaries and PR description drafts from earlier development phases have been moved out of the repository root and consolidated here for posterity:

**[history/](./history/README.md)** — `IMPLEMENTATION_SUMMARY*.md` and `PR_DESC*.md` files (issues #626–#841). These are no longer actively referenced; see merged PRs in GitHub for full context.

---

New to the repo? Read [development-setup.md](./development-setup.md) first, then [architecture.md](./architecture.md), then [adr/README.md](./adr/README.md) for the reasoning behind the current structure.

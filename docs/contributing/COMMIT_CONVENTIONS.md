# Commit Message Conventions

This project follows [Conventional Commits](https://www.conventionalcommits.org/) for automated semantic versioning and changelog generation.

## Format

```
<type>(<scope>): <subject>

<body>

<footer>
```

## Types

- **feat**: A new feature (triggers MINOR version bump)
- **fix**: A bug fix (triggers PATCH version bump)
- **docs**: Documentation only changes
- **style**: Code style changes (formatting, missing semicolons, etc.)
- **refactor**: Code changes that neither fix bugs nor add features
- **perf**: Performance improvements
- **test**: Adding or updating tests
- **build**: Changes to build system or dependencies
- **ci**: Changes to CI/CD configuration
- **chore**: Other changes that don't modify src or test files
- **revert**: Reverts a previous commit

## Breaking Changes

Add `BREAKING CHANGE:` in the footer or `!` after type to trigger MAJOR version bump:

```
feat!: remove deprecated API endpoints

BREAKING CHANGE: /auth/legacy endpoint has been removed
```

## Scope

The scope field is optional but **strongly encouraged**. It must be one of the values listed in the **Allowed Scopes** table in [CONTRIBUTING.md](../../CONTRIBUTING.md#allowed-scopes). Commitlint enforces this list — invalid scopes cause the commit to be rejected by the husky pre-commit hook.

> **Important**: commitlint splits scope values on `/`, so contract scopes use
> hyphens: `contracts-token` (not `contracts/token`).

Quick reference of common scopes:

| Category | Examples |
| -------- | -------- |
| Domain | `auth`, `courses`, `users`, `stellar`, `notifications` |
| Workspace | `backend`, `frontend`, `sdk`, `types` |
| Contracts | `contracts-analytics`, `contracts-token`, `contracts-certificate` |
| Tooling | `ci`, `deps`, `docker`, `security`, `docs` |

For the canonical list, see [CONTRIBUTING.md § Allowed Scopes](../../CONTRIBUTING.md#allowed-scopes).

## Examples

### Feature

```
feat(courses): add video upload support

Implement video upload endpoint with S3 integration
```

### Bug Fix

```
fix(auth): prevent token refresh race condition

Add mutex lock to refresh token validation
```

### Documentation

```
docs(contracts-certificate): update credential issuance flow

Add curl examples for the /v1/credentials endpoint
```

### Dependency Update

```
chore(deps): upgrade bcrypt to v6.0.0

bcrypt v5.1.x -> v6.0.0 drops @mapbox/node-pre-gyp dependency,
resolving tar@6 security advisory (GHSA-xxx).
```

### Breaking Change

```
feat(api)!: migrate to v2 API structure

BREAKING CHANGE: All endpoints now require /v2 prefix
```

### Smart Contract Change

```
fix(contracts-token): prevent double-mint on concurrent requests

Apply reentrancy guard from brain-storm-shared to mint() entrypoint
```

## Enforcement

Commits are validated using `commitlint` via husky `commit-msg` hook. Invalid commits are rejected with an error message listing the allowed scopes.

To test a message before committing:

```bash
echo "feat(auth): add OAuth2 support" | npx commitlint
```

## References

- [Conventional Commits](https://www.conventionalcommits.org/)
- [Semantic Versioning](https://semver.org/)
- [Release Please](https://github.com/googleapis/release-please)
- [CONTRIBUTING.md Allowed Scopes](../../CONTRIBUTING.md#allowed-scopes)

# Contract Upgrade Process

Soroban contracts on Stellar can be upgraded in-place via `env.deployer().update_current_contract_wasm()`.
`SharedContract` performs that swap only through its **timelocked** upgrade flow
(`schedule_upgrade` → `execute_upgrade`), gated behind admin authentication.

> **#1169:** the old immediate `SharedContract::upgrade(admin, new_wasm_hash)` entry point
> swapped the WASM without a timelock, a pending record or an audit trail, so it bypassed the
> intended authorization flow. It now refuses the swap (`"Direct upgrade disabled: use
> schedule_upgrade then execute_upgrade"`); only the timelocked flow changes contract code.

## How it works

1. Build the new contract WASM and get its hash (uploaded to the network via `soroban contract upload`).
2. `SharedContract::schedule_upgrade(admin, new_wasm_hash, timelock_ledgers)` — the stored admin must
   sign; the all-zero WASM hash and a zero-length timelock are rejected.
3. Wait `timelock_ledgers` ledgers. `cancel_upgrade(admin)` aborts the pending upgrade at any point
   before execution.
4. `SharedContract::execute_upgrade(admin)` — after the timelock, the admin executes. The upgrade is
   written to the immutable upgrade history, a `("upg_exec", "hash")` event is emitted, and the
   contract code is replaced atomically; storage is preserved.

`governance`-managed contracts follow the longer form documented in
[contract-interfaces.md](./contract-interfaces.md#upgrade-guide):
`propose_upgrade` → `vote_upgrade` → `approve_upgrade` (stored admin only) → `execute_upgrade`.

## CLI example

```bash
# 1. Upload new WASM and capture the hash
NEW_HASH=$(soroban contract upload \
  --wasm target/wasm32-unknown-unknown/release/brain_storm_shared.wasm \
  --source admin_key \
  --network testnet)

# 2. Schedule the upgrade with a timelock (1440 ledgers ≈ 1 hour)
soroban contract invoke \
  --id $CONTRACT_ID \
  --source admin_key \
  --network testnet \
  -- schedule_upgrade \
  --admin $ADMIN_ADDRESS \
  --new_wasm_hash $NEW_HASH \
  --timelock_ledgers 1440

# 3. After the timelock expires, execute it
soroban contract invoke \
  --id $CONTRACT_ID \
  --source admin_key \
  --network testnet \
  -- execute_upgrade \
  --admin $ADMIN_ADDRESS
```

## Security notes / authorization invariants (#1169)

- **Only the stored admin can schedule, execute or cancel.** The check lives inside
  `shared::upgrade` itself (`access::require_admin`), so no entry point or future internal call can
  reach the upgrade logic without it: an unauthorized caller is rejected with
  `"Unauthorized: admin required"` even if it holds other roles or partial multisig approvals.
- **Authentication *and* authorization are both required.** `admin.require_auth()` proves the admin
  key signed; the stored-admin equality check rejects impersonation.
- **The admin slot is written exactly once** — `initialize` refuses a second call, so nobody can
  become admin after deployment and then upgrade the contract.
- **No timelock bypass.** The only path to `update_current_contract_wasm` is
  `execute_upgrade`, which requires a pending upgrade whose `execute_after` ledger has passed; a
  zero-length timelock is rejected at schedule time.
- **Bad targets are rejected up front.** Scheduling the all-zero WASM hash is refused, and
  `governance::propose_upgrade` refuses the all-zero target address.
- Always verify the new WASM hash matches your audited build before broadcasting.

## Testing

The upgrade path is tested in `contracts/shared/src/upgrade_tests.rs` (timelock, history, state
preservation) and `contracts/shared/src/tests.rs` (auth guards), including the #1169 cases:
unauthorized/role-based/partial-multisig attempts, module-level bypass attempts, zero hash and zero
timelock. Because `update_current_contract_wasm` requires a real WASM hash on-chain, the swap
itself should be exercised against a local Soroban sandbox using `soroban-cli`.

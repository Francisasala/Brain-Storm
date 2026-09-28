# Brain-Storm Smart Contract Reference

Complete interface documentation for all Soroban smart contracts on the Stellar network.

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Analytics Contract](#analytics-contract)
3. [Token Contract (BST)](#token-contract-bst)
4. [Certificate Contract](#certificate-contract)
5. [Badge Contract](#badge-contract)
6. [Governance Contract](#governance-contract)
7. [Reputation Contract](#reputation-contract)
8. [Scholarship Fund Contract](#scholarship-fund-contract)
9. [Liquidity Pool Contract](#liquidity-pool-contract)
10. [NFT Contract](#nft-contract)
11. [Market Contract](#market-contract)
12. [Credential Metadata Contract](#credential-metadata-contract)
13. [Registry Contract](#registry-contract)
14. [Dispute Contract](#dispute-contract)
15. [Grants Contract](#grants-contract)
16. [Royalty Distribution Contract](#royalty-distribution-contract)
17. [Buyback Contract](#buyback-contract)
18. [Token Restrictions Contract](#token-restrictions-contract)
19. [Shared / RBAC Contract](#shared--rbac-contract)
20. [`contracts/integration` — Not a Contract](#contractsintegration--not-a-contract)
21. [Cross-Contract Call Conventions](#cross-contract-call-conventions)
22. [Worked Examples](#worked-examples)
23. [Security Considerations](#security-considerations)
24. [Upgrade Guide](#upgrade-guide)

> This reference covers all 19 crates under `contracts/`. For _why_ the platform is split into this many contracts instead of fewer, see [docs/adr/ADR-006](./adr/ADR-006-contract-per-domain-architecture.md) through [ADR-009](./adr/ADR-009-credential-nft-decomposition.md).

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    Brain-Storm Contracts                     │
│                                                             │
│  ┌───────────┐   ┌──────────┐   ┌──────────────────────┐   │
│  │ Analytics │   │  Token   │   │     Certificate      │   │
│  │  (progress│   │  (BST)   │   │   (soulbound NFT)    │   │
│  │   & stats)│   │          │   │                      │   │
│  └─────┬─────┘   └────┬─────┘   └──────────────────────┘   │
│        │              │                                     │
│  ┌─────▼─────┐   ┌────▼──────┐   ┌──────────────────────┐  │
│  │   Badge   │   │Governance │   │      Reputation      │  │
│  │           │   │           │   │                      │  │
│  └───────────┘   └───────────┘   └──────────────────────┘  │
│                                                             │
│  ┌──────────────────┐   ┌────────────────────────────────┐  │
│  │  Scholarship Fund│   │       Liquidity Pool           │  │
│  └──────────────────┘   └────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

All contracts follow the same initialization pattern:

- `initialize(admin)` — one-time setup, panics if called again
- `get_admin()` / `set_admin(new_admin)` — admin key rotation

---

## Analytics Contract

Tracks per-student course progress and emits Soroban events for off-chain indexers.

### Functions

| Function                                                    | Auth   | Description                         |
| ----------------------------------------------------------- | ------ | ----------------------------------- |
| `initialize(admin)`                                         | none   | One-time setup                      |
| `set_admin(new_admin)`                                      | admin  | Transfer admin role                 |
| `get_admin()`                                               | —      | Read current admin                  |
| `record_progress(caller, student, course_id, progress_pct)` | caller | Record/update progress (0–100)      |
| `reset_progress(admin, student, course_id)`                 | admin  | Reset a student's progress          |
| `get_progress(student, course_id)`                          | —      | Read a progress record              |
| `get_all_progress(student)`                                 | —      | All progress records for a student  |
| `get_completed_courses(student)`                            | —      | Completed courses list              |
| `get_in_progress_courses(student)`                          | —      | In-progress courses list            |
| `get_progress_paginated(student, start, limit)`             | —      | Paginated progress records          |
| `get_progress_above_threshold(student, threshold)`          | —      | Records above a progress %          |
| `count_completed_courses(student)`                          | —      | Count of completions                |
| `get_average_progress(student)`                             | —      | Average progress across all courses |
| `get_milestone(student, course_id, milestone_pct)`          | —      | Read a milestone record             |
| `get_achieved_milestones(student, course_id)`               | —      | All achieved milestones             |
| `get_total_students()`                                      | —      | Total students tracked              |
| `get_total_courses()`                                       | —      | Total courses tracked               |
| `get_completion_stats()`                                    | —      | Aggregate completion statistics     |
| `get_daily_stats(day)`                                      | —      | Stats for a specific day            |
| `get_weekly_stats(week)`                                    | —      | Stats for a specific week           |
| `get_monthly_stats(month)`                                  | —      | Stats for a specific month          |
| `get_top_performers(limit)`                                 | —      | Top students by completion count    |
| `update_aggregates(admin)`                                  | admin  | Recalculate aggregate stats         |

### Events

| Topics                       | Data                                  | Condition                          |
| ---------------------------- | ------------------------------------- | ---------------------------------- |
| `("analytics", "prog_upd")`  | `(student, course_id, progress_pct)`  | every `record_progress` call       |
| `("analytics", "completed")` | `(student, course_id)`                | when `progress_pct == 100`         |
| `("analytics", "milestone")` | `(student, course_id, milestone_pct)` | when a milestone is first achieved |

### Usage Example

```bash
stellar contract invoke \
  --id $ANALYTICS_CONTRACT_ID \
  --source backend-keypair \
  --network testnet \
  -- record_progress \
  --caller $STUDENT_ADDRESS \
  --student $STUDENT_ADDRESS \
  --course_id RUST101 \
  --progress_pct 75
```

---

## Token Contract (BST)

ERC-20-compatible fungible token with vesting, staking, airdrop, and burn mechanics.

### Functions

| Function                                                        | Auth        | Description                             |
| --------------------------------------------------------------- | ----------- | --------------------------------------- |
| `initialize(admin, name, symbol, decimals, initial_supply)`     | none        | Deploy and mint initial supply to admin |
| `mint(admin, to, amount)`                                       | admin       | Mint new tokens                         |
| `burn(from, amount)`                                            | from        | Burn tokens (updates burn stats)        |
| `transfer(from, to, amount)`                                    | from        | Transfer tokens                         |
| `approve(owner, spender, amount, expiry)`                       | owner       | Set allowance                           |
| `transfer_from(spender, from, to, amount)`                      | spender     | Transfer using allowance                |
| `balance(account)`                                              | —           | Read balance                            |
| `allowance(owner, spender)`                                     | —           | Read allowance                          |
| `total_supply()`                                                | —           | Read total supply                       |
| `get_burn_stats()`                                              | —           | Cumulative burn data                    |
| `create_vesting(admin, beneficiary, amount, start, cliff, end)` | admin       | Create vesting schedule                 |
| `claim_vesting(beneficiary, schedule_id)`                       | beneficiary | Claim vested tokens                     |
| `get_vesting(beneficiary, schedule_id)`                         | —           | Read vesting schedule                   |

### Staking (via `staking` module)

| Function                           | Auth | Description               |
| ---------------------------------- | ---- | ------------------------- |
| `stake(user, amount, lock_period)` | user | Stake BST tokens          |
| `unstake(user)`                    | user | Unstake after lock period |
| `claim_staking_rewards(user)`      | user | Claim accrued rewards     |
| `get_stake(user)`                  | —    | Read stake record         |

### Airdrop (via `airdrop` module)

| Function                                                       | Auth    | Description                          |
| -------------------------------------------------------------- | ------- | ------------------------------------ |
| `create_airdrop(admin, total, per_claim, merkle_root, expiry)` | admin   | Set up airdrop                       |
| `claim_airdrop(claimer, proof)`                                | claimer | Claim from airdrop with Merkle proof |

---

## Certificate Contract

Issues soulbound (non-transferable) NFT certificates upon course completion.

### Functions

| Function                                                      | Auth  | Description                                |
| ------------------------------------------------------------- | ----- | ------------------------------------------ |
| `initialize(admin)`                                           | none  | One-time setup                             |
| `set_admin(new_admin)`                                        | admin | Transfer admin role                        |
| `get_admin()`                                                 | —     | Read admin                                 |
| `mint_certificate(admin, recipient, course_id, metadata_url)` | admin | Issue certificate; returns `cert_id`       |
| `get_certificate(id)`                                         | —     | Read a certificate by ID                   |
| `get_certificates_by_owner(owner)`                            | —     | All certificates for an address            |
| `revoke_certificate(admin, cert_id, reason)`                  | admin | Revoke a certificate                       |
| `is_revoked(cert_id)`                                         | —     | Check revocation status                    |
| `get_revocation(cert_id)`                                     | —     | Read revocation details                    |
| `transfer(...)`                                               | —     | Always panics — certificates are soulbound |

### Events

| Topics               | Data                         | Condition     |
| -------------------- | ---------------------------- | ------------- |
| `("cert", "mint")`   | `(id, recipient, course_id)` | on mint       |
| `("cert", "revoke")` | `(id, reason)`               | on revocation |

### Usage Example

```bash
# Mint a certificate
stellar contract invoke \
  --id $CERTIFICATE_CONTRACT_ID \
  --source admin \
  --network testnet \
  -- mint_certificate \
  --admin $ADMIN_ADDRESS \
  --recipient $STUDENT_ADDRESS \
  --course_id RUST101 \
  --metadata_url "https://api.brain-storm.com/v1/certs/1"
```

---

## Badge Contract

Issues achievement badges (non-transferable) tied to badge type definitions.

### Functions

| Function                                                            | Auth  | Description                          |
| ------------------------------------------------------------------- | ----- | ------------------------------------ |
| `initialize(admin)`                                                 | none  | One-time setup                       |
| `get_admin()`                                                       | —     | Read admin                           |
| `create_badge_type(admin, badge_type, name, description, criteria)` | admin | Define a new badge type              |
| `get_badge_type(badge_type)`                                        | —     | Read badge type definition           |
| `mint_badge(admin, recipient, badge_type)`                          | admin | Issue badge; returns `badge_id`      |
| `get_badge(id)`                                                     | —     | Read badge by ID                     |
| `get_badges_by_owner(owner)`                                        | —     | All badges for an address            |
| `verify_badge(owner, badge_type)`                                   | —     | Check if owner holds a badge type    |
| `transfer(...)`                                                     | —     | Always panics — badges are soulbound |

---

## Governance Contract

On-chain proposal voting and contract upgrade governance.

### Functions

| Function                                                    | Auth     | Description                                       |
| ----------------------------------------------------------- | -------- | ------------------------------------------------- |
| `initialize(admin, token_contract)`                         | none     | One-time setup; links BST token for voting weight |
| `get_admin()`                                               | —        | Read admin                                        |
| `create_proposal(proposer, title, description, voting_end)` | proposer | Submit a proposal                                 |
| `vote(voter, proposal_id, support)`                         | voter    | Cast vote (true = for, false = against)           |
| `execute_proposal(proposal_id)`                             | —        | Execute a passed proposal after voting ends       |
| `get_proposal(proposal_id)`                                 | —        | Read proposal details                             |
| `has_voted(proposal_id, voter)`                             | —        | Check if address voted                            |
| `propose_upgrade(proposer, new_wasm_hash, description)`     | proposer | Propose contract upgrade                          |
| `vote_upgrade(voter, upgrade_id, support)`                  | voter    | Vote on upgrade proposal                          |
| `approve_upgrade(admin, upgrade_id)`                         | admin    | Admin approval gate                               |
| `execute_upgrade(upgrade_id)`                               | —        | Execute approved upgrade                          |
| `get_upgrade_proposal(upgrade_id)`                          | —        | Read upgrade proposal                             |

---

## Reputation Contract

Tracks on-chain reputation scores with decay mechanics and threshold gating.

### Functions

| Function                                        | Auth  | Description                                 |
| ----------------------------------------------- | ----- | ------------------------------------------- |
| `initialize(admin)`                             | none  | One-time setup                              |
| `get_admin()`                                   | —     | Read admin                                  |
| `update_reputation(admin, user, delta, reason)` | admin | Add or subtract reputation points           |
| `get_reputation(user)`                          | —     | Read current score                          |
| `get_reputation_record(user)`                   | —     | Full record with metadata                   |
| `get_reputation_level(user)`                    | —     | Level (0–5) derived from score              |
| `apply_decay(admin, user)`                      | admin | Apply time-based decay to a user            |
| `set_decay_config(admin, rate, period)`         | admin | Configure decay parameters                  |
| `get_decay_config()`                            | —     | Read current decay config                   |
| `claim_reputation_reward(user)`                 | user  | Claim token reward for reputation milestone |
| `verify_reputation_threshold(user, min_score)`  | —     | Boolean gate check                          |
| `verify_reputation_level(user, min_level)`      | —     | Boolean gate check                          |
| `get_reputation_history(user, start, limit)`    | —     | Paginated update history                    |
| `get_total_reputation()`                        | —     | Sum of all reputation scores                |

---

## Scholarship Fund Contract

Community-funded scholarships with application and approval workflow.

### Functions

| Function                                                      | Auth      | Description                   |
| ------------------------------------------------------------- | --------- | ----------------------------- |
| `initialize(admin)`                                           | none      | One-time setup                |
| `donate(donor, amount)`                                       | donor     | Donate BST tokens to the fund |
| `apply_for_scholarship(applicant, course_id, amount, reason)` | applicant | Submit application            |
| `approve_application(admin, app_id)`                          | admin     | Approve and disburse tokens   |
| `reject_application(admin, app_id)`                           | admin     | Reject application            |

---

## Liquidity Pool Contract

AMM-style BST/XLM liquidity pool with LP mining rewards.

### Functions

| Function                                          | Auth | Description                              |
| ------------------------------------------------- | ---- | ---------------------------------------- |
| `initialize(admin, token_a, token_b, fee_bps)`    | none | One-time setup                           |
| `add_liquidity(user, amount_a, amount_b, min_lp)` | user | Provide liquidity; receive LP tokens     |
| `remove_liquidity(user, lp_amount, min_a, min_b)` | user | Redeem LP tokens for underlying          |
| `swap(user, token_in, amount_in, min_amount_out)` | user | Swap tokens via constant-product formula |
| `claim_mining_rewards(user)`                      | user | Claim accrued LP mining rewards          |
| `get_pool_stats()`                                | —    | Reserves, fee rate, total LP             |
| `get_user_liquidity(user)`                        | —    | User's LP token balance                  |
| `get_swap_history(start_index, limit)`            | —    | Paginated swap records                   |

---

## NFT Contract

Generic, tradeable course NFT with marketplace listing and per-holder access grants. Unlike the soulbound `certificate`/`badges` contracts, ownership is transferable by default and the contract includes its own escrow-free marketplace (`list_nft`/`buy_nft`/`delist_nft`).

### Functions

| Function                                                                                           | Auth      | Description                                                                                                |
| -------------------------------------------------------------------------------------------------- | --------- | ---------------------------------------------------------------------------------------------------------- |
| `initialize(admin)`                                                                                | none      | One-time setup                                                                                             |
| `get_admin()`                                                                                      | —         | Read admin                                                                                                 |
| `mint_course_nft(admin, owner, course_id, course_name, instructor, purchase_price, royalty_basis)` | admin     | Mint an NFT; returns `nft_id`. Called directly, or cross-contract by `credential_metadata::issue_with_nft` |
| `transfer_nft(from, to, nft_id)`                                                                   | from      | Transfer ownership                                                                                         |
| `grant_access(nft_owner, nft_id, holder)`                                                          | nft_owner | Grant a non-owner address access to gated content tied to the NFT                                          |
| `revoke_access(nft_owner, nft_id, holder)`                                                         | nft_owner | Revoke a previously-granted access                                                                         |
| `has_access(nft_id, holder)`                                                                       | —         | Check access                                                                                               |
| `get_nft_metadata(nft_id)`                                                                         | —         | Read metadata                                                                                              |
| `get_nft_owner(nft_id)`                                                                            | —         | Read current owner                                                                                         |
| `get_owner_nfts(owner)`                                                                            | —         | All NFT IDs owned by an address                                                                            |
| `get_royalty_info(nft_id)`                                                                         | —         | `(instructor, royalty_basis)` for a given NFT                                                              |
| `burn_nft(owner, nft_id)`                                                                          | owner     | Destroy an NFT                                                                                             |
| `list_nft(seller, nft_id, price)`                                                                  | seller    | List an owned NFT for sale                                                                                 |
| `delist_nft(seller, nft_id)`                                                                       | seller    | Cancel a listing                                                                                           |
| `buy_nft(buyer, nft_id)`                                                                           | buyer     | Purchase a listed NFT; transfers ownership and payment                                                     |
| `get_listing(nft_id)`                                                                              | —         | Read listing details                                                                                       |

### Events

Topic symbols emitted (grep-verified against `contracts/nft/src/lib.rs`): `minted`, `xfer`, `acc_grt`, `acc_rvk`, `burned`, `listed`, `delisted`, `sold`, all under the `nft` prefix.

### Notes

`nft` performs no on-chain cross-contract calls of its own — it is called _into_ by `credential_metadata` (see [Cross-Contract Call Conventions](#cross-contract-call-conventions)) and separately, as an independent contract, by `apps/backend` for plain marketplace flows. See [ADR-009](./adr/ADR-009-credential-nft-decomposition.md) for why this is a separate contract from `certificate` and `credential_metadata`.

---

## Market Contract

Escrow, tips, protocol fees, and multi-sig escrow for marketplace-style payments. Does not hold NFTs or credentials itself — `apps/backend` composes `market` with `nft` for a full purchase flow (see [Worked Example 1](#worked-example-1-marketplace-purchase-market--nft--royalty_distribution)).

### Functions

| Function                                                                    | Auth                   | Description                                                                                                 |
| --------------------------------------------------------------------------- | ---------------------- | ----------------------------------------------------------------------------------------------------------- |
| `initialize(admin)`                                                         | none                   | One-time setup                                                                                              |
| `get_admin()`                                                               | —                      | Read admin                                                                                                  |
| `pause(admin)` / `unpause(admin)` / `is_paused()`                           | admin                  | Emergency stop for all mutating operations (#663)                                                           |
| `set_fee_bps(admin, fee_bps)` / `get_fee_bps()`                             | admin                  | Configure protocol fee, in basis points                                                                     |
| `set_treasury(admin, treasury)` / `get_treasury_balance()`                  | admin                  | Configure the fee-collection treasury address                                                               |
| `fund_escrow(payer, payee, amount)`                                         | payer                  | Fund a simple escrow; returns `escrow_id`. Blocked when paused                                              |
| `settle_escrow(caller, escrow_id)`                                          | payer or admin         | Apply fee to treasury, pay net to payee. Blocked when paused                                                |
| `refund_escrow(admin, escrow_id)`                                           | admin                  | Refund escrow to payer. Blocked when paused                                                                 |
| `get_escrow(escrow_id)`                                                     | —                      | Read escrow state                                                                                           |
| `tip(tipper, amount)`                                                       | tipper                 | Send a tip; fee → treasury, net amount returned to caller. Blocked when paused                              |
| `batch_settle_escrows(caller, escrow_ids)`                                  | admin or payer of each | Settle multiple escrows in one transaction (#662). Blocked when paused                                      |
| `batch_refund_escrows(admin, escrow_ids)`                                   | admin                  | Refund multiple escrows in one transaction (#662). Blocked when paused                                      |
| `ms_fund_escrow(payer, payee, amount, signers, threshold, timeout_ledgers)` | payer                  | Fund a multi-signature escrow (#658) requiring `threshold`-of-`signers` approval                            |
| `ms_approve_escrow(escrow_id, signer)`                                      | signer                 | Approve a multi-sig escrow                                                                                  |
| `ms_timeout_escrow(escrow_id)`                                              | —                      | Mark expired if the approval threshold wasn't met in time; caller/backend then triggers the refund fallback |
| `ms_get_escrow(escrow_id)`                                                  | —                      | Read multi-sig escrow state                                                                                 |

### Events

Topic symbols emitted: `paused`, `unpaused`, `es_fund`, `es_settl`, `es_refnd`, `tip`, all under the `market` prefix.

### Notes

`market` performs no on-chain cross-contract calls — it moves value entirely within its own escrow storage. Payment amounts are caller-supplied `i128` values; `market` does not itself invoke `token::transfer`. Composing an actual token/XLM payment with escrow funding/settlement is the caller's (i.e. `apps/backend`'s) responsibility. See [Cross-Contract Call Conventions](#cross-contract-call-conventions).

---

## Credential Metadata Contract

Off-chain-content metadata and lifecycle (expiry/renewal/content-hash verification) for a credential, optionally linked atomically to an `nft` at issuance time. See [ADR-009](./adr/ADR-009-credential-nft-decomposition.md) for why this is separate from `certificate` and `nft`.

### Functions

| Function                                                                                                                                              | Auth  | Description                                                                                                                             |
| ----------------------------------------------------------------------------------------------------------------------------------------------------- | ----- | --------------------------------------------------------------------------------------------------------------------------------------- |
| `initialize(admin)`                                                                                                                                   | none  | One-time setup, no NFT linkage support                                                                                                  |
| `initialize_with_nft(admin, nft_contract)`                                                                                                            | admin | One-time setup, registers the `nft` contract address for linkage (#635)                                                                 |
| `issue_with_nft(admin, credential_id, course_name, completion_date, expiry_timestamp, grade, ipfs_hash, owner, course_id, instructor, royalty_basis)` | admin | Store metadata **and** atomically cross-call `nft::mint_course_nft`; rolls back entirely on either failure. Returns the minted `nft_id` |
| `get_credential_link(credential_id)`                                                                                                                  | —     | Read the `CredentialNftLink` for a credential, if any                                                                                   |
| `get_nft_credential_id(nft_id)`                                                                                                                       | —     | Reverse lookup: NFT → credential                                                                                                        |
| `credential_is_linked(credential_id)`                                                                                                                 | —     | Check whether a credential has a linked NFT                                                                                             |
| `store_metadata(admin, credential_id, course_name, completion_date, expiry_timestamp, grade, ipfs_hash)`                                              | admin | Store metadata **without** minting an NFT                                                                                               |
| `update_metadata(admin, credential_id, course_name, grade)`                                                                                           | admin | Update mutable fields of an existing record                                                                                             |
| `get_metadata(credential_id)`                                                                                                                         | —     | Read the full metadata record                                                                                                           |
| `is_expired(credential_id)` / `is_valid(credential_id)` / `can_renew(credential_id)`                                                                  | —     | Lifecycle state checks                                                                                                                  |
| `renew_credential(admin, credential_id, new_expiry_timestamp)`                                                                                        | admin | Extend expiry                                                                                                                           |
| `emit_expiry_event(credential_id)`                                                                                                                    | —     | Emit an expiry event for off-chain indexers                                                                                             |
| `store_metadata_hash(admin, credential_id, hash)`                                                                                                     | admin | Store a content hash (e.g. of the full credential document) for later verification                                                      |
| `verify_metadata_hash(credential_id, hash)`                                                                                                           | —     | Compare a supplied hash against the stored one                                                                                          |
| `get_metadata_history(credential_id, index)` / `get_history_count(credential_id)`                                                                     | —     | Paginated update history                                                                                                                |

### Events

Topic symbols emitted: `cred` (linkage), `store`, `update`, `renew`, `expire`.

### Notes

`issue_with_nft` is one of only three verified on-chain cross-contract calls in the entire contract suite — see [Worked Example 2](#worked-example-2-credential-issuance-with-linked-nft-635) and [ADR-006](./adr/ADR-006-contract-per-domain-architecture.md#verified-on-chain-call-graph).

---

## Registry Contract

Verification levels, certified skills (with expiry), specialisations, curator permissions, and a paginated global user directory (#656).

### Functions

| Function                                                                                              | Auth             | Description                                                                                          |
| ----------------------------------------------------------------------------------------------------- | ---------------- | ---------------------------------------------------------------------------------------------------- |
| `initialize(admin)`                                                                                   | none             | One-time setup                                                                                       |
| `get_admin()`                                                                                         | —                | Read admin                                                                                           |
| `pause(admin)` / `unpause(admin)` / `is_paused()`                                                     | admin            | Emergency stop                                                                                       |
| `add_curator(admin, curator)` / `remove_curator(admin, curator)` / `is_curator(addr)`                 | admin            | Manage the curator set — curators can set verification levels/skills/specialisations alongside admin |
| `set_verification_level(setter, user, level)` / `get_verification_level(user)`                        | admin or curator | Set/read a user's verification level. Blocked when paused                                            |
| `add_certified_skill(setter, user, skill, expiry_ts)` / `remove_certified_skill(setter, user, skill)` | admin or curator | Manage certified skills. Blocked when paused                                                         |
| `get_certified_skills(user)`                                                                          | —                | Returns only non-expired skills                                                                      |
| `has_certified_skill(user, skill)`                                                                    | —                | Boolean check                                                                                        |
| `set_specialisations(setter, user, specs)` / `get_specialisations(user)`                              | admin or curator | Manage specialisations. Blocked when paused                                                          |
| `batch_register_users(users)`                                                                         | each user        | Register multiple users in one transaction. Blocked when paused                                      |
| `batch_set_verification_levels(setter, users, level)`                                                 | admin or curator | Bulk-set verification levels. Blocked when paused                                                    |
| `register_user(user)`                                                                                 | user             | Idempotently register a user in the global directory                                                 |
| `list_users(offset, limit)` / `list_users_by_level(min_level, offset, limit)` / `total_users()`       | —                | Paginated directory reads                                                                            |

### Notes

`registry` performs no on-chain cross-contract calls. It is unrelated to `contracts/integration` despite the similar-sounding names — see [ADR-008](./adr/ADR-008-registry-integration-separation.md).

---

## Dispute Contract

Escrow dispute resolution with an Open → Evidence → Decision → Settled lifecycle (#659).

### Functions

| Function                                        | Auth                   | Description                                                                                       |
| ----------------------------------------------- | ---------------------- | ------------------------------------------------------------------------------------------------- |
| `initialize(admin, arbiter)`                    | admin                  | One-time setup, sets the arbiter address                                                          |
| `get_arbiter()` / `set_arbiter(admin, arbiter)` | admin (set)            | Read/rotate the arbiter                                                                           |
| `open_dispute(claimant, respondent, amount)`    | claimant               | Open a dispute over a given amount; returns `dispute_id`                                          |
| `submit_evidence(caller, dispute_id, hash)`     | claimant or respondent | Submit an evidence hash, moves the dispute to the Evidence phase                                  |
| `record_decision(arbiter, dispute_id, outcome)` | arbiter                | Record the arbiter's ruling, moves to Decision phase                                              |
| `settle(arbiter, dispute_id)`                   | arbiter                | Enforce the ruling; computes and returns `(claimant_amount, respondent_amount)`, moves to Settled |
| `get_dispute(dispute_id)`                       | —                      | Read dispute state                                                                                |

### Notes

`dispute` computes payout splits but does not itself move tokens — like `market`, actual fund transfer is the caller's responsibility. It performs no on-chain cross-contract calls.

---

## Grants Contract

Milestone-based grant applications with admin approval and BST fund release.

### Functions

| Function                                                                                                                             | Auth      | Description                                                                |
| ------------------------------------------------------------------------------------------------------------------------------------ | --------- | -------------------------------------------------------------------------- |
| `initialize(admin, token_contract)`                                                                                                  | admin     | One-time setup; records the `token` contract address used for fund release |
| `get_admin()`                                                                                                                        | —         | Read admin                                                                 |
| `apply_for_grant(applicant, title, description, total_amount, milestone_count)`                                                      | applicant | Submit an application; returns `grant_id`                                  |
| `approve_grant(admin, grant_id)` / `reject_grant(admin, grant_id)`                                                                   | admin     | Approve or reject an application                                           |
| `set_milestone(admin, grant_id, milestone_idx, description, amount)`                                                                 | admin     | Define a milestone's payout amount                                         |
| `release_milestone_funds(admin, grant_id, milestone_idx)`                                                                            | admin     | Cross-calls `token::transfer` to pay the applicant the milestone amount    |
| `submit_report(applicant, grant_id, content)`                                                                                        | applicant | Submit a progress report                                                   |
| `get_grant(grant_id)` / `get_milestone(grant_id, milestone_idx)` / `get_grant_reports(grant_id)` / `get_applicant_grants(applicant)` | —         | Reads                                                                      |

### Notes

`release_milestone_funds` is one of only three verified on-chain cross-contract calls in the suite (`env.invoke_contract` to `token::transfer`) — see [ADR-006](./adr/ADR-006-contract-per-domain-architecture.md#verified-on-chain-call-graph) and [Cross-Contract Call Conventions](#cross-contract-call-conventions).

---

## Royalty Distribution Contract

Configurable creator/contributor/platform royalty splits per course, with a pull-based withdrawal model.

### Functions

| Function                                                                                      | Auth      | Description                                                                                 |
| --------------------------------------------------------------------------------------------- | --------- | ------------------------------------------------------------------------------------------- |
| `initialize(admin)`                                                                           | admin     | One-time setup                                                                              |
| `set_royalty_split(admin, course_id, creator_pct, contributor_pct, platform_pct)`             | admin     | Define the percentage split for a course (must sum to 100)                                  |
| `add_royalty_recipient(admin, course_id, recipient)`                                          | admin     | Register a recipient address for a course's contributor share                               |
| `distribute_royalties(admin, course_id, total_amount)`                                        | admin     | Record a distribution event, crediting each recipient's pull-balance according to the split |
| `withdraw_royalties(recipient)`                                                               | recipient | Pull the caller's accrued balance                                                           |
| `get_royalty_balance(recipient)`                                                              | —         | Read a recipient's withdrawable balance                                                     |
| `get_royalty_split(course_id)`                                                                | —         | Read a course's configured split                                                            |
| `get_payment_record(payment_id)` / `get_payment_count()` / `get_total_distributed(course_id)` | —         | Payment history reads                                                                       |

### Notes

`royalty_distribution` performs no on-chain cross-contract calls; `distribute_royalties` records amounts in its own storage rather than moving tokens directly — `apps/backend` (or an admin) is responsible for funding the contract's notion of "distributed" amounts and for the actual token movement backing a withdrawal. **Build note:** this crate's `Cargo.toml` exists under `contracts/royalty_distribution/` but, unlike the other 18 contracts, is not currently listed in the root `Cargo.toml` workspace `members` — see [ADR-006](./adr/ADR-006-contract-per-domain-architecture.md#context). Build it explicitly:

```bash
cargo build --manifest-path contracts/royalty_distribution/Cargo.toml --target wasm32-unknown-unknown
```

---

## Buyback Contract

Automated BST buyback-and-burn mechanism, triggered by a configurable price threshold via an oracle and DEX pool reference.

### Functions

| Function                                                                                                    | Auth  | Description                                                                                                   |
| ----------------------------------------------------------------------------------------------------------- | ----- | ------------------------------------------------------------------------------------------------------------- |
| `initialize(admin, token_contract, oracle_contract, dex_contract, dex_pool_id)`                             | admin | One-time setup; records the token, oracle, and DEX contract addresses used for buyback decisions              |
| `update_config(admin, enabled, price_threshold, max_buyback_amount, min_reserve_balance, buyback_interval)` | admin | Update any subset of buyback parameters                                                                       |
| `get_config()`                                                                                              | —     | Read current configuration                                                                                    |
| `check_and_execute_buyback()`                                                                               | —     | Callable by anyone (e.g. a scheduled off-chain job); executes a buyback only if configured conditions are met |
| `manual_buyback(admin, max_xlm_amount)`                                                                     | admin | Force a buyback outside the automated schedule                                                                |
| `add_to_reserve(from, amount)`                                                                              | from  | Fund the reserve used for buybacks                                                                            |
| `get_reserve_balance()` / `get_buyback_analytics()` / `get_buyback_history(start_index, limit)`             | —     | Reads                                                                                                         |

### Notes

`buyback` records `oracle_contract` and `dex_contract` addresses at `initialize` time for future price-check/swap integration, but as of this writing `check_and_execute_buyback`/`manual_buyback` do not perform a verified on-chain `invoke_contract` call to those addresses (no `invoke_contract` usage found in `contracts/buyback/src/lib.rs`) — treat the oracle/DEX wiring as configuration state rather than an active on-chain integration until confirmed otherwise by a maintainer familiar with this contract's latest state.

---

## Token Restrictions Contract

Whitelist/blacklist, per-account transfer limits, and an emergency-override switch — a policy layer intended to sit alongside `token`, not inside it.

### Functions

| Function                                                                                                         | Auth  | Description                                                                     |
| ---------------------------------------------------------------------------------------------------------------- | ----- | ------------------------------------------------------------------------------- |
| `initialize(admin)`                                                                                              | admin | One-time setup                                                                  |
| `add_to_whitelist(admin, account)` / `remove_from_whitelist(admin, account)` / `is_whitelisted(account)`         | admin | Manage whitelist                                                                |
| `add_to_blacklist(admin, account)` / `remove_from_blacklist(admin, account)` / `is_blacklisted(account)`         | admin | Manage blacklist                                                                |
| `set_transfer_limit(admin, account, limit)` / `get_transfer_limit(account)`                                      | admin | Per-account transfer cap                                                        |
| `request_transfer_approval(from, to, amount)`                                                                    | from  | Request approval for a transfer that would otherwise be restricted              |
| `approve_transfer(admin, from, to)` / `is_transfer_approved(from, to)`                                           | admin | Approve/check a pending transfer request                                        |
| `activate_emergency_override(admin)` / `deactivate_emergency_override(admin)` / `is_emergency_override_active()` | admin | Bypass all restriction checks in an emergency                                   |
| `can_transfer(from, to)`                                                                                         | —     | Composite check: not blacklisted, within limits or approved, or override active |
| `get_restriction_log(log_id)` / `get_log_count()`                                                                | —     | Audit log of restriction decisions                                              |

### Notes

`token_restrictions` is a **policy contract, not the token itself** — it does not hold or move BST balances (no `invoke_contract` to `token` was found). `can_transfer` is a read-only advisory check; the caller (`apps/backend` or a future `token` integration) is responsible for consulting it before calling `token::transfer`. As of this writing, `contracts/token/src/lib.rs`'s `transfer` does not itself call `can_transfer` — the two contracts are not yet wired together on-chain.

---

## Shared / RBAC Contract

Role-based access control library used by other contracts for cross-contract authorization checks.

The crate serves two roles. `SharedContract` (behind the default-on `contract` Cargo feature) is the deployable RBAC contract. The rest of the crate is a plain library that other contracts link against with `default-features = false`:

| Module       | Provides                                                                                                   |
| ------------ | ---------------------------------------------------------------------------------------------------------- |
| `access`     | Admin/authority/owner checks — see [Auth conventions](#auth-conventions)                                   |
| `math`       | Overflow-checked `i128`/`u32` arithmetic, including `checked_mul_div_i128` for proportional-share formulas |
| `validation` | Positive-amount, percentage-bound, and future-timestamp guards                                             |
| `pausable`   | Pause flag with auto-unpause                                                                               |
| `reentrancy` | Reentrancy lock                                                                                            |
| `multisig`   | N-of-M proposal flow                                                                                       |
| `upgrade`    | Timelocked WASM upgrade flow                                                                               |
| `errors`     | `SharedError` — defined, not yet consumed (see issue #822)                                                 |

---

## `contracts/integration` — Not a Contract

`contracts/integration` has no `src/lib.rs` and no `#[contract]` struct — it is a `[dev-dependencies]`-only crate (path-depending on `brain-storm-analytics`, `brain-storm-token`, and `brain-storm-shared`) whose sole purpose is `tests/integration.rs`: deploying those three contracts into one Soroban test `Env` and scripting an end-to-end register → progress → reward flow. It is never compiled to WASM or deployed. See [ADR-008](./adr/ADR-008-registry-integration-separation.md) for the full rationale, and `contracts/integration/README.md` for how to run it locally (`cargo test --test integration -- --test-threads=1` against a local Stellar sandbox).

---

## Cross-Contract Call Conventions

Verified by grepping every `contracts/*/src/*.rs` for `invoke_contract` and `#[contractclient]` (the only two ways a Soroban contract calls another contract in this codebase). There are exactly **three** on-chain cross-contract call edges across all 18 production contracts — everywhere else, multi-contract flows are composed off-chain by `apps/backend` issuing separate transactions. Full rationale: [ADR-006](./adr/ADR-006-contract-per-domain-architecture.md#verified-on-chain-call-graph).

### Auth conventions

- Every state-mutating function takes the acting party's `Address` as an explicit parameter and calls `<address>.require_auth()` as its first statement (e.g. `admin.require_auth()`, `payer.require_auth()`, `nft_owner.require_auth()`). There is no implicit `msg.sender`-style caller identity — the caller is always passed explicitly and Soroban verifies the corresponding signature was authorized for this invocation.
- Read-only query functions (`get_*`, `is_*`, `has_*`, `list_*`) take no auth and require no `require_auth()` call.
- Admin-gated functions additionally compare the passed address against a stored `Admin` (or, in `registry`, `Admin`-or-curator-set) value **after** calling `require_auth()`: `require_auth()` proves the caller controls that address; the storage comparison proves that address is _allowed_ to perform the action. Both checks are required — `require_auth()` alone does not enforce authorization.
- Since issue #825 both halves live in [`contracts/shared/src/access.rs`](../contracts/shared/src/access.rs) rather than being copied inline. Use `access::require_admin(&env, &caller, &DataKey::Admin)` in new code; it takes the storage key as a generic parameter so each contract keeps its own `DataKey` enum. The other helpers are `require_authority` (a non-admin authority slot such as `dispute`'s arbiter), `require_admin_or` (admin _or_ a named party, e.g. an escrow payer), `require_owner` (per-resource ownership), `is_admin` (predicate, no auth and no panic — for building compound checks), and `read_authority` (the raw read, with a `"Not initialized"` message instead of a bare `.unwrap()`).
- Admin-check failures panic with `"Unauthorized: admin required"`, authority-slot failures with `"Unauthorized: authority required"`, and ownership failures with `"Unauthorized: owner required"`. `registry` and `reputation` keep their own compound messages.
- Depend on `brain-storm-shared` with `default-features = false`. Its `contract` feature compiles `SharedContract`'s `#[contractimpl]` block, which — if linked into another contract's wasm — would export `assign_role`, `upgrade`, and `pause_contract` from that contract, acting on its storage.
- `market`, `registry`, and `shared` each implement their own local `pause`/`unpause`/`is_paused` — there is no shared on-chain pause registry. See [ADR-007](./adr/ADR-007-shared-crate-for-common-code.md) for why this pattern is currently copied per-contract rather than centralized.

### Error propagation across contract boundaries

- Contracts in this codebase primarily use `assert!`/`panic!`/`.expect(...)` with a string message rather than the `#[contracterror]` enum pattern (only `contracts/shared/src/errors.rs` defines a `SharedError` enum, and it is not consumed by other contracts' error paths).
- A panic anywhere inside a Soroban invocation — including inside a cross-contract call made via `invoke_contract` or a `#[contractclient]` stub — aborts and rolls back the **entire** transaction, including any storage writes already made earlier in the same invocation. This is what `credential_metadata::linkage::issue_and_mint_nft`'s doc comment means by "If this panics, the whole invocation rolls back — no partial state": if `nft::mint_course_nft` panics, the `credential_metadata` record it was about to link is never written either.
- Callers cannot catch or recover from a callee's panic within the same transaction — there is no try/catch equivalent across a Soroban cross-contract call. If a flow needs "best effort, continue on failure" semantics (as opposed to "all or nothing"), it must be implemented as separate transactions orchestrated off-chain by `apps/backend`, not as a single invocation spanning multiple contracts.

### Shared token-interface assumptions

- `governance` and `grants` both call into `token` via **untyped** `env.invoke_contract(&token_contract, &symbol_short!("balance" | "transfer"), args)` rather than a generated typed client. This means neither contract depends on `token`'s crate at compile time, but also means neither gets compile-time verification that `token`'s `balance`/`transfer` signature hasn't changed — a `token` interface change could silently break `governance` or `grants` at runtime. When changing `token::balance` or `token::transfer`'s signature, grep `contracts/governance/src/lib.rs` and `contracts/grants/src/lib.rs` for `invoke_contract` and update the call sites manually.
- `credential_metadata`'s call into `nft` uses a typed but **locally-declared** `#[contractclient]` stub (`contracts/credential_metadata/src/linkage.rs`'s `nft_contract_client` module) rather than importing the real `brain-storm-nft` crate — the stub's doc comment explains this is deliberate: "we declare a minimal stub here so the credential_metadata crate compiles without depending on the nft crate directly." If `nft::mint_course_nft`'s signature changes, this stub must be updated by hand to match; it will not fail to compile if it drifts, only fail at runtime.
- No contract in this codebase assumes a SEP-0041-standard `Client` for calling `token` generically — only the exact `balance`/`transfer` function names and argument shapes `governance`/`grants` already use are relied upon.

---

## Worked Examples

### Worked Example 1: Marketplace Purchase (`market` + `nft` + `royalty_distribution`)

There is no single on-chain "buy" call spanning these three contracts — `apps/backend` sequences separate transactions:

```
Buyer clicks "Buy" on a listed course NFT
        │
        ▼
Backend: POST /v1/market/purchase  (reads nft.get_listing(nft_id) for price)
        │
        ├─ Tx 1 — MarketContract.fund_escrow(buyer, seller, price)
        │           (buyer signs; escrow now holds the funds)
        │
        ├─ Tx 2 — MarketContract.settle_escrow(caller, escrow_id)
        │           (fee → treasury, net → seller's pending balance)
        │
        ├─ Tx 3 — NFTContract.buy_nft(buyer, nft_id)
        │           (ownership transfers to buyer)
        │
        └─ Tx 4 — RoyaltyDistributionContract.distribute_royalties(admin, course_id, net_amount)
                    (credits creator/contributor/platform pull-balances per the configured split)
        │
        ▼
Backend records the purchase (txHashes, nft_id, buyer) in PostgreSQL
```

Because these are four separate Stellar transactions rather than one atomic Soroban invocation, `apps/backend` — not any contract — is responsible for detecting and reconciling partial failure (e.g. escrow settled but the royalty-distribution transaction fails). This is the direct consequence of the "off-chain composition by default" decision in [ADR-006](./adr/ADR-006-contract-per-domain-architecture.md).

### Worked Example 2: Credential Issuance with Linked NFT (#635)

This is one of the three cases where atomicity is enforced **on-chain**, inside a single Soroban invocation:

```
Admin issues a credential with NFT linkage
        │
        ▼
Backend: POST /v1/credentials/issue?withNft=true
        │
        ▼
CredentialMetadataContract.issue_with_nft(
    admin, credential_id, course_name, completion_date,
    expiry_timestamp, grade, ipfs_hash, owner, course_id,
    instructor, royalty_basis
)
        │
        ├─ 1. Stores the credential metadata record in credential_metadata's own storage
        │
        ├─ 2. Cross-contract call via the local `nft_contract_client::Client` stub:
        │       NFTContract.mint_course_nft(admin, owner, course_id, course_name,
        │                                    instructor, purchase_price=0, royalty_basis)
        │      — if this panics, step 1's write is also rolled back (whole tx aborts)
        │
        ├─ 3. Stores the bidirectional CredentialNftLink (credential_id ↔ nft_id)
        │
        └─ 4. Emits ("linked", "cred") event with (credential_id, nft_id, owner)
        │
        ▼
Backend records credential_id + nft_id in PostgreSQL (single tx hash)
        │
        ▼
Student dashboard: GET /v1/credentials/:userId
    ├── CredentialMetadataContract.get_metadata(credential_id)
    └── CredentialMetadataContract.get_credential_link(credential_id) → NFTContract.get_nft_metadata(nft_id)
```

Unlike Worked Example 1, this whole sequence from step 1–4 is **one Soroban transaction** — see [Cross-Contract Call Conventions](#cross-contract-call-conventions) for why a panic at step 2 guarantees step 1 never persists.

---

## Security Considerations

### Admin Key Management

- Admin addresses are stored in persistent contract storage.
- Use a hardware wallet or multi-sig Stellar account for the admin key.
- Rotate the admin key periodically via `set_admin`.

### Soulbound Tokens

Certificate and Badge contracts disable `transfer` by always panicking. This prevents secondary-market circumvention of credentials.

### Reentrancy Protection

The Token contract uses a `Locked` storage key as a reentrancy guard around state-mutating operations.

### Integer Overflow

All arithmetic uses Soroban SDK types with `overflow-checks = true` in the release profile (see `Cargo.toml`).

### Storage TTL

Persistent storage entries use `TTL_THRESHOLD` / `TTL_EXTEND_TO` ledger constants. Callers must ensure entry TTLs are extended for long-lived data (e.g., via `extend_ttl`).

### Event Integrity

Events are emitted by the contract address. Off-chain indexers should verify the emitting contract ID against the known deployed address before trusting event data.

---

## Upgrade Guide

Brain-Storm contracts use Soroban's built-in `update_current_contract_wasm` mechanism gated through the Governance contract.

### Process

1. **Build new WASM** — run `./scripts/build.sh` and note the new hash.
2. **Submit upgrade proposal** — call `propose_upgrade(proposer, new_wasm_hash, description)` on the Governance contract.
3. **Community voting** — token holders call `vote_upgrade(voter, upgrade_id, support)` during the voting window.
4. **Admin approval** — admin calls `approve_upgrade(admin, upgrade_id)` if quorum is reached.
5. **Execute upgrade** — anyone calls `execute_upgrade(upgrade_id)`; the on-chain WASM is atomically replaced.
6. **Verify** — run `stellar contract info --id <CONTRACT_ID> --network testnet` to confirm the new hash.

### Storage Migration

If the new WASM introduces new storage keys, initialize them in the contract's first invocation after upgrade. Existing keys remain untouched.

### Rollback

Soroban does not natively support rollback. Keep the previous WASM hash and re-submit a new upgrade proposal pointing to it if a critical bug is discovered.

See also: [Smart Contract Upgrade Guide](./smart-contract-upgrade-guide.md)

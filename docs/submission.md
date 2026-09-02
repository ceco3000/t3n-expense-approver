# T3N ADK Bounty Submission — Confidential Expense Approver Agent

**Author:** ceco3000 · **Date:** 2026-09-02 · **Testnet:** live-verified

## What this is

An enterprise expense-approval agent whose policy evaluation runs **inside the T3N TEE**:
submissions are validated in-enclave (amount bands, category allowlist, duplicate detection),
PII is hashed at a single choke point before any storage, and only decisions, reasons,
and audit digests ever cross back to the caller.

**Why useful:** every company has an expense flow; most of them ship employee PII
(names, bank details, receipts) through regular servers. This agent demonstrates the
T3N value proposition — confidential computation with agent identity — on a real,
maintainable business workflow, with zero third-party API dependencies (fully
deterministic, demoable, and cheap to keep running post-challenge).

## Live artifacts (testnet)

- GitHub repo (public): https://github.com/ceco3000/t3n-expense-approver
- Tenant DID: `did:t3n:a47d9e420654150ffb664311f886990e60a51037`
- Contract: `z:a47d9e420654150ffb664311f886990e60a51037:expense-ledger` v0.1.2 (contract id 861)
- Agent card (ERC-8004, hosted + published on T3N): https://cn-api.sg.testnet.t3n.terminal3.io/api/agent-card/did:t3n:a47d9e420654150ffb664311f886990e60a51037
- KV map: `z:<tid>:expense-ledger` — private; writers/readers = contract 861

## How it works

- **submit-expense(input)**: validates category allowlist `{meals, transport, accommodation, software, office}`, amount bands (≤$200 auto-approve; ≤$1000 pending/manual review; >$1000 rejected; hard ceiling $10,000), and duplicate detection via `sha256(receipt_ref)`. Persists an anonymized record — employee and receipt hashed, never raw.
- **list-approvals()**: range-scans the ledger and returns anonymized records (no PII) for reporting.
- PII handling is centralized in one function (`hash_pii`), so auditing one function audits every PII exit.
- Policy constants are versioned in one place (expense.rs) for easy maintenance.

## Screenshots

[Screenshot 1: contract unit tests 6/6 passed]
[Screenshot 2: contract registration — contract id 861]
[Screenshot 3: live TEE execution — 5 scenarios verified]

## Live verification (2026-09-02, testnet)

| Scenario | Input | Result |
|---|---|---|
| Small expense | $45 meals, fresh receipt | approved |
| Medium expense | $600 transport, fresh receipt | pending → manual review |
| Oversized expense | $2,500 software, fresh receipt | rejected (policy ceiling) |
| Duplicate receipt | same receipt as scenario 1 | rejected (duplicate detected; same record hash) |
| Ledger readback | list-approvals | records returned newest-first, fully anonymized |

## Bugs found while building (reported to T3N docs team via this submission)

1. **SDK ≥ 5.3.0 rejects the cn-api trust manifest** — `fetchTrustedManifest("testnet")` fails with "Trust manifest ... is malformed" on SDK 5.3.0–5.6.0. Root cause (source-level): `isSignedTrustManifest` requires a third string-array field that cn-api's manifest (signed 2026-08-27) does not include. Pinned SDK 5.2.0 as a workaround. Full write-up in BUG_REPORT.md.
2. **KV map deletion never completes** — after `maps.delete`, the map stays in `deleting` state indefinitely (>2 min), blocking re-create of the same name. Worked around with a fresh map name (`expense-ledger`).

## Maintenance & post-challenge running

- I would like to keep running this agent; it costs nothing outside testnet credits and the scenario runner is deterministic.
- v0.2 plan: per-tenant policy overrides via a `z:<tid>:expense-policy` KV map (the hook already exists in the code), plus an optional webhook notify step using `http-with-placeholders` so PII stays out of WASM memory end-to-end.

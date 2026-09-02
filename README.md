# T3N Enterprise Agent: Confidential Expense Approver

Superteam bounty submission — "Try out new docs to build a trusted agent with Terminal 3".

A confidential enterprise expense-approval agent running inside the **T3N TEE**:
expense submissions are validated in-enclave (amount bands, category allowlist,
duplicate detection); PII is hashed at a single choke point before any storage;
only decisions, reasons, and audit digests cross back to callers.

## Live artifacts (testnet)

| Artifact | Value |
|---|---|
| Tenant DID | `did:t3n:a47d9e420654150ffb664311f886990e60a51037` |
| Contract | `z:a47d9e420654150ffb664311f886990e60a51037:expense-ledger` v0.1.2 (contract id 861) |
| Agent card | https://cn-api.sg.testnet.t3n.terminal3.io/api/agent-card/did:t3n:a47d9e420654150ffb664311f886990e60a51037 |
| KV map | `z:<tid>:expense-ledger` (private; writers/readers = contract 861) |

## Structure

- `expense-contract/` — Rust TEE contract (WIT + wit-bindgen, WASI Preview 2)
- `quickstart.ts` — tenant session setup + contract registration
- `test-invoke.ts` — end-to-end scenario runner (5 scenarios)
- `agent-card.json` — ERC-8004 agent card (hosted on T3N)
- `BUG_REPORT.md` — 2 platform bugs found during the build

## Run

```bash
set -a; source .env; set +a
npx tsx quickstart.ts      # authenticate + register contract
npx tsx test-invoke.ts     # 5-scenario demo
```

## Bugs found (see BUG_REPORT.md)

1. SDK ≥5.3.0 rejects the cn-api trust manifest (schema drift) — pinned 5.2.0
2. KV map deletion stuck in "deleting" — worked around via new map name

//! z-expense-approver v0.1.0 — confidential enterprise expense approval.
//!
//! An enterprise expense-approval agent whose policy evaluation runs inside
//! the T3N TEE:
//!   - `submit-expense`: validates a submission against the policy (amount
//!     bands, category allowlist, duplicate detection) entirely inside the
//!     enclave. PII-bearing fields (`employee_id`, `receipt_ref`) are
//!     hashed before anything is persisted. Only the decision, reasons,
//!     and an audit digest cross back to the caller.
//!   - `list-approvals`: range-scans the approval ledger and returns
//!     anonymized records (no PII) for reporting.
//!
//! # Storage layout (z: KV maps, names derived at runtime)
//!   - `z:<tid>:expense-ledger` — key: `<sha256(receipt_ref)>` value: JSON
//!     record `{record_id, decision, category, amount_usd, submitted_at, digest}`
//!   - `z:<tid>:expense-policy`    — optional policy override, key `policy`,
//!     JSON `{auto_approve_cap_usd, pending_ceiling_usd, categories: [...]}`
//!
//! # Maintenance notes
//!   - Policy constants are versioned in ONE place (`expense.rs`).
//!   - All PII handling is centralized in `hash_pii()` — audit that function
//!     and you have audited every PII exit.
//!   - No external HTTP calls in v0.1 — the contract is fully deterministic
//!     and demoable without third-party credentials.
#![warn(clippy::style, missing_debug_implementations)]
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

extern crate alloc;

pub const CONTRACT_VERSION: &str = "0.1.0";

wit_bindgen::generate!({
    world: "expense-approver",
    path: "wit",
    additional_derives: [
        serde::Deserialize,
        serde::Serialize,
    ],
    generate_all,
});

mod expense;

struct Component;

#[cfg(target_arch = "wasm32")]
impl exports::z::expense_approver::contracts::Guest for Component {
    fn submit_expense(
        req: exports::z::expense_approver::contracts::GenericInput,
    ) -> Result<alloc::vec::Vec<u8>, alloc::string::String> {
        let input = req.input.ok_or("submit-expense: missing input")?;
        expense::submit_expense(&input)
    }

    fn list_approvals(
        req: exports::z::expense_approver::contracts::GenericInput,
    ) -> Result<alloc::vec::Vec<u8>, alloc::string::String> {
        let input = req.input.ok_or("list-approvals: missing input")?;
        expense::list_approvals(&input)
    }
}

#[cfg(target_arch = "wasm32")]
export!(Component);

#[cfg(test)]
mod tests {
    use super::CONTRACT_VERSION;

    #[test]
    fn contract_version_is_semver() {
        let parts: Vec<&str> = CONTRACT_VERSION.split('.').collect();
        assert_eq!(parts.len(), 3, "CONTRACT_VERSION must be MAJOR.MINOR.PATCH");
        for part in parts {
            assert!(part.parse::<u32>().is_ok(), "each part must be a number");
        }
    }
}

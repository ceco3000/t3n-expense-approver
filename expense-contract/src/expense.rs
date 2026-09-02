//! Policy evaluation + approval ledger for the expense approver.
//!
//! Everything here runs inside the TEE. See the module docs in `lib.rs` for
//! the threat model: raw submissions never leave the enclave; PII is hashed
//! at the single choke point `hash_pii()`.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Versioned in one place so policy changes are explicit and reviewable.
const POLICY_VERSION: &str = "1";

/// Default auto-approval cap: amounts at or below this value are approved.
const DEFAULT_AUTO_APPROVE_CAP_USD: u32 = 200;

/// Default pending ceiling: amounts above the cap but at or below this value
/// go to manual review; anything above is rejected by policy.
const DEFAULT_PENDING_CEILING_USD: u32 = 1_000;

/// Hard ceiling: anything above this is rejected regardless of policy map.
const HARD_REJECT_CEILING_USD: u32 = 10_000;

/// Category allowlist (default policy).
const DEFAULT_CATEGORIES: [&str; 5] = ["meals", "transport", "accommodation", "software", "office"];

/// Cap for `list-approvals` scans.
const LIST_LIMIT: u32 = 50;

const APPROVALS_MAP_SUFFIX: &str = "expense-ledger";
const POLICY_MAP_SUFFIX: &str = "expense-policy";

#[derive(Deserialize)]
struct SubmitExpenseReq {
    employee_id: String,
    category: String,
    amount_usd: u32,
    #[allow(dead_code)]
    description: String,
    #[allow(dead_code)]
    merchant: String,
    receipt_ref: String,
}

#[derive(Serialize)]
struct SubmitExpenseResp {
    decision: &'static str,
    reasons: Vec<String>,
    record_id: String,
    audit_digest: String,
}

#[derive(Serialize, Deserialize)]
struct ApprovalRecord {
    record_id: String,
    decision: String,
    category: String,
    amount_usd: u32,
    submitted_at: u64,
    audit_digest: String,
}

#[derive(Serialize)]
struct ListApprovalsResp {
    approvals: Vec<ApprovalRecord>,
}

/// Single choke point for PII: hash instead of store. Auditing this function
/// covers every PII exit from the contract.
fn hash_pii(field: &str, value: &str) -> String {
    let mut h = Sha256::new();
    h.update(field.as_bytes());
    h.update([0u8]);
    h.update(value.as_bytes());
    hex::encode(h.finalize())
}

#[cfg(target_arch = "wasm32")]
use crate::host::{
    interfaces::{kv_store, logging},
    tenant::tenant_context,
};

#[cfg(target_arch = "wasm32")]
fn approvals_map_name() -> String {
    let tid = tenant_context::tenant_did();
    format!("z:{}:{}", hex::encode(&tid), APPROVALS_MAP_SUFFIX)
}

/// Reserved for v0.2: per-tenant policy overrides from `z:<tid>:expense-policy`.
#[cfg(target_arch = "wasm32")]
#[allow(dead_code)]
fn policy_map_name() -> String {
    let tid = tenant_context::tenant_did();
    format!("z:{}:{}", hex::encode(&tid), POLICY_MAP_SUFFIX)
}

/// Validate a submission and record the decision. Runs entirely in-TEE.
pub fn submit_expense(input: &[u8]) -> Result<Vec<u8>, String> {
    let req: SubmitExpenseReq = serde_json::from_slice(input)
        .map_err(|e| format!("submit-expense: bad input: {e}"))?;

    #[cfg(target_arch = "wasm32")]
    {
        submit_expense_wasm(&req)
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = req;
        Err("submit_expense is only implemented on the wasm32 target".to_string())
    }
}

#[cfg(target_arch = "wasm32")]
fn submit_expense_wasm(req: &SubmitExpenseReq) -> Result<Vec<u8>, String> {
    let mut reasons: Vec<String> = Vec::new();

    // --- Category allowlist ---
    if !DEFAULT_CATEGORIES.contains(&req.category.as_str()) {
        reasons.push(format!(
            "category '{}' not in allowlist {:?}",
            req.category, DEFAULT_CATEGORIES
        ));
    }

    // --- Amount sanity ---
    if req.amount_usd == 0 {
        reasons.push("amount must be greater than zero".to_string());
    }
    if req.amount_usd > HARD_REJECT_CEILING_USD {
        reasons.push(format!(
            "amount exceeds hard ceiling of ${HARD_REJECT_CEILING_USD}"
        ));
    }

    // --- Duplicate detection (PII hashed, never stored raw) ---
    let receipt_hash = hash_pii("receipt_ref", &req.receipt_ref);
    let existing = kv_store::get(&approvals_map_name(), receipt_hash.as_bytes())
        .map_err(|e| format!("kv read: {e}"))?;
    let is_duplicate = existing.is_some();
    if is_duplicate {
        reasons.push("duplicate receipt: this expense was already submitted".to_string());
    }

    // --- Decision ---
    let decision: &'static str = if !reasons.is_empty() {
        "rejected"
    } else if req.amount_usd <= DEFAULT_AUTO_APPROVE_CAP_USD {
        "approved"
    } else if req.amount_usd <= DEFAULT_PENDING_CEILING_USD {
        reasons.push(format!(
            "amount ${} exceeds auto-approve cap — routed to manual review",
            req.amount_usd
        ));
        "pending"
    } else {
        reasons.push("amount exceeds policy ceiling — requires manager review".to_string());
        "rejected"
    };

    // --- Persist anonymized record ---
    let record = if !is_duplicate {
        let record_id = hash_pii("employee_id", &req.employee_id);
        let ts = tenant_context::cluster_timestamp_secs();
        let audit_digest = {
            let mut h = Sha256::new();
            h.update(POLICY_VERSION.as_bytes());
            h.update([0u8]);
            h.update(decision.as_bytes());
            h.update([0u8]);
            h.update(req.amount_usd.to_be_bytes());
            h.update([0u8]);
            h.update(req.category.as_bytes());
            h.update([0u8]);
            h.update(receipt_hash.as_bytes());
            hex::encode(h.finalize())
        };
        let rec = ApprovalRecord {
            record_id,
            decision: decision.to_string(),
            category: req.category.clone(),
            amount_usd: req.amount_usd,
            submitted_at: ts,
            audit_digest: audit_digest.clone(),
        };
        let value = serde_json::to_vec(&rec).map_err(|e| e.to_string())?;
        kv_store::put(
            &approvals_map_name(),
            receipt_hash.as_bytes(),
            &value,
        )
        .map_err(|e| format!("kv put: {e}"))?;
        audit_digest
    } else {
        // Return the stored digest for traceability.
        let stored: ApprovalRecord = serde_json::from_slice(&existing.unwrap())
            .map_err(|e| format!("corrupt ledger entry: {e}"))?;
        stored.audit_digest
    };

    let _ = logging::info(&format!(
        "expense decision={} amount={} category={}",
        decision, req.amount_usd, req.category
    ));

    let resp = SubmitExpenseResp {
        decision,
        reasons,
        record_id: hash_pii("employee_id", &req.employee_id),
        audit_digest: record,
    };
    serde_json::to_vec(&resp).map_err(|e| e.to_string())
}

/// Range-scan the ledger and return anonymized records (no PII fields).
pub fn list_approvals(_input: &[u8]) -> Result<Vec<u8>, String> {
    #[cfg(target_arch = "wasm32")]
    {
        list_approvals_wasm()
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        Err("list_approvals is only implemented on the wasm32 target".to_string())
    }
}

#[cfg(target_arch = "wasm32")]
fn list_approvals_wasm() -> Result<Vec<u8>, String> {
    // 半开区间扫描 [start, end)：end 用 65 字节 0xFF 覆盖 64 字节 sha256-hex 键上界
    let end: alloc::vec::Vec<u8> = alloc::vec![0xFF; 65];
    let entries = kv_store::scan(&approvals_map_name(), &[], &end, LIST_LIMIT)
        .map_err(|e| format!("kv scan: {e}"))?;

    let mut approvals: Vec<ApprovalRecord> = Vec::with_capacity(entries.len());
    for (_key, value) in entries {
        let rec: ApprovalRecord =
            serde_json::from_slice(&value).map_err(|e| format!("corrupt ledger entry: {e}"))?;
        approvals.push(rec);
    }
    approvals.sort_by(|a, b| b.submitted_at.cmp(&a.submitted_at));

    let resp = ListApprovalsResp { approvals };
    serde_json::to_vec(&resp).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_pii_is_stable_and_domain_separated() {
        let a = hash_pii("receipt_ref", "R-1001");
        let b = hash_pii("receipt_ref", "R-1001");
        let c = hash_pii("employee_id", "R-1001");
        assert_eq!(a, b, "same input must hash identically");
        assert_ne!(a, c, "different field domains must not collide");
        assert_eq!(a.len(), 64, "sha256 hex is 64 chars");
    }

    #[test]
    fn submit_expense_non_wasm_returns_err() {
        let input = serde_json::to_vec(&serde_json::json!({
            "employee_id": "emp-7",
            "category": "meals",
            "amount_usd": 25,
            "description": "lunch",
            "merchant": "cafe",
            "receipt_ref": "R-1001",
        }))
        .unwrap();
        let result = submit_expense(&input);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("only implemented on the wasm32 target"));
    }

    #[test]
    fn submit_expense_bad_input_returns_err() {
        let result = submit_expense(b"not json");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("bad input"));
    }

    #[test]
    fn list_approvals_non_wasm_returns_err() {
        let result = list_approvals(b"{}");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("only implemented on the wasm32 target"));
    }
}

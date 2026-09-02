// test-invoke.ts — tenant 会话路径调用 expense-approver 合约
import {
  T3nClient,
  setEnvironment,
  loadWasmComponent,
  eth_get_address,
  metamask_sign,
  createEthAuthInput,
  fetchTrustedManifest,
  TenantClient,
  getNodeUrl,
} from "@terminal3/t3n-sdk";

setEnvironment("testnet");

const T3N_API_KEY = process.env.T3N_API_KEY!;
const wasmComponent = await loadWasmComponent();
const address = eth_get_address(T3N_API_KEY);

const t3n = new T3nClient({
  trustAnchor: await fetchTrustedManifest("testnet"),
  wasmComponent,
  handlers: {
    EthSign: metamask_sign(address, undefined, T3N_API_KEY),
  },
});

await t3n.handshake();
const did = await t3n.authenticate(createEthAuthInput(address));
const tenantDid = did.value;
console.log("Connected as:", tenantDid);

const tenant = new TenantClient({
  t3n,
  baseUrl: getNodeUrl(),
  tenantDid,
});

const TAIL = "expense-ledger";

// 重建 KV map（合约 858/860 可读写）
try {
  await tenant.maps.create({
    tail: TAIL,
    visibility: "private",
    writers: { only: [861] },
    readers: { only: [861] },
  });
  console.log("Map created: expense-approvals");
} catch (e: any) {
  // 删除是异步的：等待后重试
  for (let i = 0; i < 5; i++) {
    await new Promise((r) => setTimeout(r, 3000));
    try {
      await tenant.maps.create({
        tail: TAIL,
        visibility: "private",
        writers: { only: [861] },
        readers: { only: [861] },
      });
      console.log("Map created (retry", i + 1, ")");
      break;
    } catch (e2: any) {
      console.log("Map create retry", i + 1, ":", e2?.message?.slice(0, 60) || e2);
    }
  }
}

async function call(fn: string, input: unknown) {
  return tenant.contracts.execute(TAIL, {
    version: "0.1.2",
    functionName: fn,
    input,
  });
}

// 场景 1：小额 → approved
const r1 = await call("submit-expense", {
  employee_id: "emp-001",
  category: "meals",
  amount_usd: 45,
  description: "team lunch",
  merchant: "cafe-88",
  receipt_ref: "R-2026-0001",
});
console.log("1. 小额报销:", JSON.stringify(r1));

// 场景 2：中额 → pending
const r2 = await call("submit-expense", {
  employee_id: "emp-002",
  category: "transport",
  amount_usd: 600,
  description: "client visit taxi",
  merchant: "city-cab",
  receipt_ref: "R-2026-0002",
});
console.log("2. 中额报销:", JSON.stringify(r2));

// 场景 3：超额 → rejected
const r3 = await call("submit-expense", {
  employee_id: "emp-003",
  category: "software",
  amount_usd: 2500,
  description: "gpu server",
  merchant: "cloud-co",
  receipt_ref: "R-2026-0003",
});
console.log("3. 超额报销:", JSON.stringify(r3));

// 场景 4：重复提交 → duplicate
const r4 = await call("submit-expense", {
  employee_id: "emp-001",
  category: "meals",
  amount_usd: 45,
  description: "team lunch (dup)",
  merchant: "cafe-88",
  receipt_ref: "R-2026-0001",
});
console.log("4. 重复提交:", JSON.stringify(r4));

// 场景 5：审批列表
const r5 = await call("list-approvals", {});
console.log("5. 审批列表:", JSON.stringify(r5));

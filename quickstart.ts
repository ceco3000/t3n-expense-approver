// T3N Quickstart — 首次认证调用（官方文档代码）
import {
  T3nClient,
  setEnvironment,
  loadWasmComponent,
  eth_get_address,
  metamask_sign,
  createEthAuthInput,
  fetchTrustedManifest,
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
console.log("Address:", address);

// ===== Set Up Dev Env: TenantClient =====
import { TenantClient, getNodeUrl } from "@terminal3/t3n-sdk";

const tenant = new TenantClient({
  t3n,
  baseUrl: getNodeUrl(),
  tenantDid,
});

await tenant.tenant.me();
console.log("TenantClient ready.");

// ===== Step 3: Register the TEE contract =====
import { readFile } from "fs/promises";

const WASM_PATH = "./expense-contract/target/wasm32-wasip2/release/z_expense_approver.wasm";
const CONTRACT_TAIL = "expense-ledger";
const CONTRACT_VERSION = "0.1.2";

const wasmBytes = await readFile(WASM_PATH);
const result = await tenant.contracts.register({
  tail: CONTRACT_TAIL,
  version: CONTRACT_VERSION,
  wasm: wasmBytes,
});

const contractId = result.contract_id;
const tenantId = tenantDid.slice("did:t3n:".length);
const scriptName = `z:${tenantId}:${CONTRACT_TAIL}`;

console.log(`registered ${scriptName} as contract id ${contractId}`);

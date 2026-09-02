# Bug 报告：SDK ≥5.3.0 与 cn-api 端点 trust manifest schema 不同步

## 严重度：🔴 高（阻断 Quickstart，影响全部使用默认端点的新用户）

## 复现步骤

1. 安装最新 SDK：`npm install @terminal3/t3n-sdk@5.6.0`（2026-09-02 05:40 UTC 发布）
2. 按官方 Quickstart 运行 `fetchTrustedManifest("testnet")`
3. 端点：`https://cn-api.sg.testnet.t3n.terminal3.io/api/trust-manifest`（HTTP 200）

## 现象

```
Error: Trust manifest at https://cn-api.sg.testnet.t3n.terminal3.io/api/trust-manifest is malformed.
```

## 根因分析（源码级）

SDK `fetchTrustedManifest` 调用内部 `isSignedTrustManifest(manifest)` 做结构验证，要求 **8 个字段**：

| 要求 | 字段类型 |
|------|---------|
| cluster | string |
| version | number |
| 数组 A（peer_ids） | string[] |
| 数组 B（rtmr3_allowlist） | string[] |
| **数组 C（第 3 数组）** | string[] |
| signed_at | string |
| signature | string |

而 cn-api 端点当前下发的 manifest（`signed_at: 2026-08-27`）只有 **6 个字段**，缺少第 3 个数组字段：

```json
{
  "cluster": "testnet",
  "version": 1787800421,
  "peer_ids": [...3 项...],
  "rtmr3_allowlist": [...1 项...],
  "signed_at": "2026-08-27T03:13:41Z",
  "signature": "0x3873..."
}
```

**版本矩阵**（实测）：

| SDK 版本 | 发布日 | manifest 验证 |
|:--------:|:------:|:---:|
| 5.0.0 ~ 5.2.0 | ≤08-26 | ✅ 通过 |
| 5.3.0 ~ 5.6.0 | 08-28 ~ 09-02 | ❌ malformed |

**结论**：SDK 5.3.0（2026-08-28）引入了新 schema 字段要求，但 cn-api 端点仍下发旧 schema manifest（08-27 签署）。**SDK 发布与端点部署不同步**，所有使用默认 cn-api 端点的 5.3+ 用户都会被阻断在 Quickstart 第一步。

## 绕过方法

```bash
npm install @terminal3/t3n-sdk@5.2.0
```

## 建议修复

1. cn-api 端点部署新 schema manifest（含第 3 数组字段）后恢复最新 SDK
2. 或 SDK 结构验证做向后兼容（第 3 数组缺失时降级接受）
3. 两者同步发版后解除 5.2.0 锁定

## 环境

- Node v23.11.0，macOS 26.5.2
- 网络：中国大陆（端点域名 cn-api 为 SDK 默认配置，非网络问题；浏览器与 curl 双通道 manifest 内容一致，排除中间层篡改）

---

## Bug 2：KV map 删除后长时间停留在 "deleting" 状态

- **现象**：`tenant.maps.delete("expense-approvals")` 后，同名 map 创建被拒「map is being deleted; the name will be free to re-create once deletion completes」，`maps.getStatus` 持续返回 `"deleting"`，超过 2 分钟未完成
- **影响**：开发迭代时无法复用 map 名（本场景：合约版本升级导致 contract id 变化需要重建 map ACL）
- **绕过**：换新 map 名（`expense-ledger`）重新部署合约（v0.1.2，contract id 861）
- **建议修复**：文档化删除完成时间；提供阻塞式 delete 或 idempotent create

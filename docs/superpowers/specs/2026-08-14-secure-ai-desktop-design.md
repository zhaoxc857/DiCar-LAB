# DiCar Tune 安全 AI 桌面通道设计

## 1. 目标与边界

0.2.0 的 AI 调参只在 Windows Tauri 桌面版可用。桌面 WebView 不再直接访问 DeepSeek，也不再把 API Key 存在 localStorage。Rust 固定访问官方 `https://api.deepseek.com/chat/completions`，前端继续通过 `AiChatClient` 使用自动调参引擎。

本设计不支持自定义 API 地址、浏览器 AI、真实 DeepSeek 自动化测试、云端密钥同步或其他平台凭据库。模型名仍由用户输入。

## 2. Rust 服务边界

新增独立于设备 `AppState` 的 `AiServiceState`，由 Tauri Builder 管理。它拥有复用的 `reqwest::Client`、Windows 凭据适配器，以及 `request_id -> CancellationToken` 活动请求表。

依赖固定为：

- `reqwest 0.12`，关闭默认特性，启用 JSON 与 rustls TLS；
- `keyring 3.6.3`，关闭默认特性，仅启用 `windows-native`；
- `tokio 1` 与 `tokio-util 0.7`，用于异步选择和取消。

HTTP 客户端禁用重定向，连接超时 10 秒，总超时 60 秒。响应使用分块读取并在超过 1 MiB 前终止。请求最多 32 条消息，全部内容不超过 64 KiB；模型名 trim 后为 1–64 个 `[A-Za-z0-9._:-]` 字符。

凭据项使用 service `com.dicar.tune`、user `deepseek-api-key`。Key trim 后必须为 1–512 字节且不含控制字符。前端只能读取 `configured: boolean`，绝不能取回 Key。

## 3. Tauri 命令

- `ai_credential_status() -> Result<AiCredentialStatusDto, AiErrorDto>`
- `ai_set_api_key(api_key: String) -> Result<(), AiErrorDto>`
- `ai_clear_api_key() -> Result<(), AiErrorDto>`
- `ai_complete(request: AiCompletionRequestDto) -> Result<String, AiErrorDto>`
- `ai_cancel(request_id: String) -> Result<(), AiErrorDto>`

`AiCompletionRequestDto` 包含 UUID 字符串 `requestId`、`model` 和 `messages`。重复 request ID 被拒绝。`ai_complete` 在读取凭据后注册 token，以 `select` 等待 HTTP 或取消，所有结束路径都从活动表移除。`ai_cancel` 幂等：已结束或未知 ID 也返回成功。

错误码稳定区分 `aiUnavailable`、`aiKeyMissing`、`aiInvalidRequest`、`aiCancelled`、`aiTimeout`、`aiHttpError`、`aiResponseTooLarge`、`aiInvalidResponse` 与 `credentialStoreError`。错误信息不得包含 Authorization、Key 或完整提示词。

## 4. 前端服务与迁移

新增 `AiPlatform`：

```ts
export interface AiPlatform {
  readonly available: boolean;
  getCredentialStatus(): Promise<{ configured: boolean }>;
  setApiKey(apiKey: string): Promise<void>;
  clearApiKey(): Promise<void>;
  createClient(model: string): AiChatClient;
}
```

`TauriAiPlatform` 封装 invoke；`UnavailableAiPlatform` 在非 Tauri 环境报告不可用。`TauriAiClient.complete` 生成 UUID，注册 AbortSignal 监听并调用 `ai_cancel`，把稳定错误码映射为现有中文错误。

`AppProviders` 同时提供 DesktopBridge 与 AiPlatform，并允许测试注入。React 组件不得直接 import Tauri invoke。

`settingsStore` 升级到 v3，仅持久化串口字段和 `aiModel`。迁移丢弃 `aiBaseUrl` 与 `aiApiKey`，并验证迁移后的原始 localStorage 不含密钥。向导在桌面版显示凭据状态、保存/替换/删除入口；保存成功立刻清空密码输入。在浏览器模式只显示桌面版要求。

## 5. 安全与测试

AI 仍只提出参数建议；本地范围、步长、看门狗、RAM-only 与人工固化边界保持不变。任何网络或凭据失败均沿用引擎失败/中止路径并保持本地最佳 RAM 参数。

Rust 使用内存凭据替身和本地 TCP HTTP 服务器测试成功、401/429/500、禁止重定向、超时、取消、超限和坏 JSON。前端测试命令映射、取消竞争、浏览器禁用、密钥输入清空和 v3 明文清理。自动化测试不连接真实 DeepSeek。

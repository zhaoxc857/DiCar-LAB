import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type { AiChatClient, AiChatMessage } from "./aiClient";

export const AI_DESKTOP_ONLY_MESSAGE = "AI 调参仅 Windows 桌面版可用";
export const AI_MODEL_ERROR_MESSAGE = "模型名称只能包含字母、数字、点、下划线、冒号或连字符，且最多 64 个字符";

export function isSafeAiModel(model: string): boolean {
  return /^[A-Za-z0-9._:-]{1,64}$/.test(model);
}

export interface AiPlatform {
  readonly available: boolean;
  getCredentialStatus(): Promise<{ configured: boolean }>;
  setApiKey(apiKey: string): Promise<void>;
  clearApiKey(): Promise<void>;
  createClient(model: string): AiChatClient;
}

export type AiInvoke = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

export class TauriAiPlatform implements AiPlatform {
  readonly available = true;

  constructor(private readonly invoke: AiInvoke = tauriInvoke as AiInvoke) {}

  getCredentialStatus(): Promise<{ configured: boolean }> {
    return this.invoke("ai_credential_status")
      .then((value) => value as { configured: boolean })
      .catch(throwPlatformError);
  }

  setApiKey(apiKey: string): Promise<void> {
    return this.invoke("ai_set_api_key", { apiKey })
      .then(() => undefined)
      .catch(() => {
        throw new Error("无法保存 API Key 到 Windows 凭据库");
      });
  }

  clearApiKey(): Promise<void> {
    return this.invoke("ai_clear_api_key").then(() => undefined).catch(throwPlatformError);
  }

  createClient(model: string): AiChatClient {
    return new TauriAiChatClient(this.invoke, model);
  }
}

class TauriAiChatClient implements AiChatClient {
  constructor(
    private readonly invoke: AiInvoke,
    private readonly model: string,
  ) {}

  async complete(messages: AiChatMessage[], signal?: AbortSignal): Promise<string> {
    if (!isSafeAiModel(this.model)) throw new Error(AI_MODEL_ERROR_MESSAGE);
    const requestId = crypto.randomUUID();
    if (signal?.aborted) {
      void this.invoke("ai_cancel", { requestId }).catch(() => undefined);
      throw new Error("AI 请求已取消");
    }

    const completion = this.invoke("ai_complete", {
      request: { requestId, model: this.model, messages },
    }).then((value) => {
      if (typeof value !== "string") throw new Error("AI 桌面通道返回了无效结果");
      return value;
    });
    if (signal === undefined) {
      return completion.catch(throwPlatformError);
    }

    let abortListener: (() => void) | undefined;
    const cancelled = new Promise<never>((_resolve, reject) => {
      abortListener = () => {
        void this.invoke("ai_cancel", { requestId }).catch(() => undefined);
        reject(new Error("AI 请求已取消"));
      };
      signal.addEventListener("abort", abortListener, { once: true });
    });
    try {
      return await Promise.race([completion.catch(throwPlatformError), cancelled]);
    } finally {
      if (abortListener !== undefined) signal.removeEventListener("abort", abortListener);
    }
  }
}

export class UnavailableAiPlatform implements AiPlatform {
  readonly available = false;

  async getCredentialStatus(): Promise<{ configured: boolean }> {
    return { configured: false };
  }

  async setApiKey(apiKey: string): Promise<void> {
    void apiKey;
    throw new Error(AI_DESKTOP_ONLY_MESSAGE);
  }

  async clearApiKey(): Promise<void> {
    throw new Error(AI_DESKTOP_ONLY_MESSAGE);
  }

  createClient(model: string): AiChatClient {
    void model;
    return {
      complete: async () => {
        throw new Error(AI_DESKTOP_ONLY_MESSAGE);
      },
    };
  }
}

function throwPlatformError(error: unknown): never {
  if (typeof error === "object" && error !== null && "message" in error && typeof error.message === "string") {
    throw new Error(error.message);
  }
  if (error instanceof Error) throw error;
  throw new Error("AI 桌面通道调用失败");
}

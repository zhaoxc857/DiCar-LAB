/** 与 OpenAI 兼容的 chat completions 客户端。密钥只存本机，不随任何遥测上传。 */

export interface AiChatMessage {
  role: "system" | "user";
  content: string;
}

export interface AiChatClient {
  complete(messages: AiChatMessage[]): Promise<string>;
}

export interface DeepSeekConfig {
  baseUrl: string;
  apiKey: string;
  model: string;
  timeoutMs?: number;
}

export const DEFAULT_AI_BASE_URL = "https://api.deepseek.com";
export const DEFAULT_AI_MODEL = "deepseek-chat";

export class DeepSeekClient implements AiChatClient {
  constructor(private readonly config: DeepSeekConfig) {}

  async complete(messages: AiChatMessage[]): Promise<string> {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), this.config.timeoutMs ?? 60_000);
    try {
      const response = await fetch(`${this.config.baseUrl.replace(/\/+$/, "")}/chat/completions`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${this.config.apiKey}`,
        },
        body: JSON.stringify({
          model: this.config.model,
          messages,
          temperature: 0,
          response_format: { type: "json_object" },
        }),
        signal: controller.signal,
      });
      if (!response.ok) {
        throw new Error(`AI 服务返回 ${response.status}：${(await response.text()).slice(0, 200)}`);
      }
      const body = (await response.json()) as { choices?: Array<{ message?: { content?: string } }> };
      const content = body.choices?.[0]?.message?.content;
      if (typeof content !== "string" || content.length === 0) throw new Error("AI 服务返回了空回复");
      return content;
    } catch (error) {
      if (error instanceof DOMException && error.name === "AbortError") throw new Error("AI 请求超时");
      throw error;
    } finally {
      clearTimeout(timer);
    }
  }
}

/** 从模型输出中提取第一个 JSON 对象（容忍代码块围栏等噪声）。 */
export function extractJsonObject(text: string): unknown {
  const start = text.indexOf("{");
  const end = text.lastIndexOf("}");
  if (start < 0 || end <= start) throw new Error("AI 回复中没有 JSON 对象");
  return JSON.parse(text.slice(start, end + 1)) as unknown;
}

/** AI 调参算法使用的最小客户端契约。具体传输由桌面 AiPlatform 提供。 */

export interface AiChatMessage {
  role: "system" | "user";
  content: string;
}

export interface AiChatClient {
  complete(messages: AiChatMessage[], signal?: AbortSignal): Promise<string>;
}

export const DEFAULT_AI_MODEL = "deepseek-chat";

/** 从模型输出中提取第一个 JSON 对象（容忍代码块围栏等噪声）。 */
export function extractJsonObject(text: string): unknown {
  const start = text.indexOf("{");
  const end = text.lastIndexOf("}");
  if (start < 0 || end <= start) throw new Error("AI 回复中没有 JSON 对象");
  return JSON.parse(text.slice(start, end + 1)) as unknown;
}

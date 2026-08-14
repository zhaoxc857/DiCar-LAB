import type { AiChatMessage } from "./aiClient";
import { TauriAiPlatform, UnavailableAiPlatform, type AiInvoke } from "./aiPlatform";

const messages: AiChatMessage[] = [{ role: "user", content: "test" }];

beforeEach(() => {
  vi.stubGlobal("crypto", { randomUUID: () => "78c7dd67-566b-4452-a14e-03c8f92f75cb" });
});

afterEach(() => {
  vi.unstubAllGlobals();
});

it("maps credential operations and completions to the Tauri AI commands", async () => {
  const invoke = vi.fn<AiInvoke>(async (command) => {
    if (command === "ai_credential_status") return { configured: true };
    if (command === "ai_complete") return "reply";
    return undefined;
  });
  const platform = new TauriAiPlatform(invoke);

  await expect(platform.getCredentialStatus()).resolves.toEqual({ configured: true });
  await platform.setApiKey("sk-secret");
  await platform.clearApiKey();
  await expect(platform.createClient("deepseek-chat").complete(messages)).resolves.toBe("reply");

  expect(invoke).toHaveBeenNthCalledWith(1, "ai_credential_status");
  expect(invoke).toHaveBeenNthCalledWith(2, "ai_set_api_key", { apiKey: "sk-secret" });
  expect(invoke).toHaveBeenNthCalledWith(3, "ai_clear_api_key");
  expect(invoke).toHaveBeenNthCalledWith(4, "ai_complete", {
    request: {
      requestId: "78c7dd67-566b-4452-a14e-03c8f92f75cb",
      model: "deepseek-chat",
      messages,
    },
  });
});

it("forwards AbortSignal cancellation to Rust and rejects promptly", async () => {
  let rejectCompletion!: (error: unknown) => void;
  const invoke = vi.fn<AiInvoke>((command) => {
    if (command === "ai_complete") {
      return new Promise((_resolve, reject) => {
        rejectCompletion = reject;
      });
    }
    return Promise.resolve(undefined);
  });
  const platform = new TauriAiPlatform(invoke);
  const controller = new AbortController();

  const pending = platform.createClient("deepseek-chat").complete(messages, controller.signal);
  controller.abort();
  await expect(pending).rejects.toThrow("AI 请求已取消");
  expect(invoke).toHaveBeenCalledWith("ai_cancel", {
    requestId: "78c7dd67-566b-4452-a14e-03c8f92f75cb",
  });

  rejectCompletion({ code: "aiCancelled", message: "AI 请求已取消" });
});

it("keeps AI unavailable outside the Windows Tauri shell", async () => {
  const platform = new UnavailableAiPlatform();

  expect(platform.available).toBe(false);
  await expect(platform.getCredentialStatus()).resolves.toEqual({ configured: false });
  await expect(platform.setApiKey("sk-secret")).rejects.toThrow("AI 调参仅 Windows 桌面版可用");
  await expect(platform.createClient("deepseek-chat").complete(messages)).rejects.toThrow(
    "AI 调参仅 Windows 桌面版可用",
  );
});

it("rejects unsafe model names before invoking Rust", async () => {
  const invoke = vi.fn<AiInvoke>().mockResolvedValue("reply");
  const platform = new TauriAiPlatform(invoke);

  await expect(platform.createClient("bad model").complete(messages)).rejects.toThrow(/模型名称/);
  expect(invoke).not.toHaveBeenCalled();
});

it("never includes the submitted key in credential errors", async () => {
  const invoke = vi.fn<AiInvoke>().mockRejectedValue(new Error("IPC rejected sk-secret"));
  const platform = new TauriAiPlatform(invoke);

  const error = await platform.setApiKey("sk-secret").catch((failure: unknown) => failure);
  expect(error).toBeInstanceOf(Error);
  expect((error as Error).message).not.toContain("sk-secret");
});

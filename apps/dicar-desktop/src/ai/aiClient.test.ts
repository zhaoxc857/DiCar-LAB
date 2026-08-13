import { DeepSeekClient } from "./aiClient";

const messages = [{ role: "user" as const, content: "test" }];

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

it("reports external cancellation separately from timeout", async () => {
  const fetchMock = vi.fn((_input: RequestInfo | URL, init?: RequestInit) =>
    new Promise<Response>((_resolve, reject) => {
      init?.signal?.addEventListener("abort", () => reject(new DOMException("aborted", "AbortError")));
    }),
  );
  vi.stubGlobal("fetch", fetchMock);
  const external = new AbortController();
  const client = new DeepSeekClient({ baseUrl: "https://example.test", apiKey: "secret", model: "test", timeoutMs: 10_000 });

  const pending = client.complete(messages, external.signal);
  external.abort();

  await expect(pending).rejects.toThrow("AI 请求已取消");
  expect(fetchMock).toHaveBeenCalledTimes(1);
});

it("retains the distinct internal timeout error", async () => {
  vi.useFakeTimers();
  vi.stubGlobal("fetch", vi.fn((_input: RequestInfo | URL, init?: RequestInit) =>
    new Promise<Response>((_resolve, reject) => {
      init?.signal?.addEventListener("abort", () => reject(new DOMException("aborted", "AbortError")));
    }),
  ));
  const client = new DeepSeekClient({ baseUrl: "https://example.test", apiKey: "secret", model: "test", timeoutMs: 50 });

  const pending = expect(client.complete(messages)).rejects.toThrow("AI 请求超时");
  await vi.advanceTimersByTimeAsync(50);

  await pending;
});

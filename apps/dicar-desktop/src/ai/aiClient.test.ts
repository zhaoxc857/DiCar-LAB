import { extractJsonObject } from "./aiClient";

it("extracts the first complete JSON object from fenced model output", () => {
  expect(extractJsonObject("```json\n{\"done\":true}\n```"))
    .toEqual({ done: true });
});

it("rejects model output without a JSON object", () => {
  expect(() => extractJsonObject("no structured response"))
    .toThrow("AI 回复中没有 JSON 对象");
});

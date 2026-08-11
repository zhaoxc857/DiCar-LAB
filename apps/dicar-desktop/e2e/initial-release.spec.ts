import { expect, test } from "playwright/test";

test("B 菜单进入 A 工作台并完成写 RAM 与固化", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "工作区" })).toBeVisible();

  await page.getByRole("link", { name: /实时调参与波形/ }).click();
  await expect(page.getByRole("heading", { name: "实时调参与波形" })).toBeVisible();
  await page.getByRole("button", { name: "连接模拟器" }).click();
  await expect(page.getByRole("button", { name: "断开设备" })).toBeVisible();

  await page.getByLabel("速度环 Kp").fill("1.8");
  await page.getByRole("button", { name: "写入 RAM" }).click();
  await expect(page.getByText("1 项待固化")).toBeVisible();

  await page.getByRole("button", { name: "审阅并固化" }).click();
  await expect(page.getByRole("dialog", { name: "固化参数修改" })).toBeVisible();
  await page.getByRole("button", { name: "固化到 Flash" }).click();
  await expect(page.getByText("0 项待固化")).toBeVisible();
});

test("Observer 只能查看波形，不能修改或固化", async ({ page }) => {
  await page.goto("/live/car-01");
  await page.getByRole("button", { name: "连接模拟器" }).click();
  await page.getByLabel("演示身份").selectOption("observer");

  await expect(page.getByText("仅观察者不能修改参数")).toBeVisible();
  await expect(page.getByLabel("速度环 Kp")).toBeDisabled();
  await expect(page.getByRole("button", { name: "审阅并固化" })).toBeDisabled();
  await expect(page.getByText("8/8 通道", { exact: true })).toBeVisible();
});

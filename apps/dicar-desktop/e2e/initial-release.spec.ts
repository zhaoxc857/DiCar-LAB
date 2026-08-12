import { expect, test, type Locator, type Page } from "playwright/test";

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

test("真实串口入口展示 HC-05 配对、传出 COM 和电平安全说明", async ({ page }) => {
  await page.goto("/");
  await page.getByLabel("连接方式").selectOption("serial");
  await page.getByLabel("硬件类型").selectOption("hc05BluetoothSpp");

  await expect(page.getByText("先在 Windows 蓝牙设置中完成配对")).toBeVisible();
  await expect(page.getByText("请选择系统创建的传出（Outgoing）COM 口")).toBeVisible();
  await expect(page.getByText(/5V MCU 发往 HC-05 RX 时必须分压/)).toBeVisible();
  await expect(page.getByRole("option", { name: "自动探测" })).toBeAttached();
});

test("波形 A/B 可纯键盘操作并在 200% 缩放下保留读数", async ({ page }) => {
  await page.goto("/live/car-01");
  await page.getByRole("button", { name: "连接模拟器" }).click();
  const region = page.getByRole("region", { name: "实时波形交互区" });
  await region.focus();

  await region.press("Enter");
  await region.press("ArrowLeft");
  await region.press("Enter");
  await region.press("ArrowRight");
  await expect(page.getByRole("status", { name: "波形游标读数" })).toContainText("Δt");
  await expect(page.getByRole("columnheader", { name: "A" })).toBeAttached();
  await expect(page.getByRole("columnheader", { name: "B" })).toBeAttached();

  await page.setViewportSize({ width: 640, height: 360 });
  await expect(page.getByRole("combobox", { name: "波形工作组" })).toBeVisible();
  await expect(page.getByRole("table")).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth)).toBe(true);

  await region.press("Escape");
  await expect(page.getByRole("status", { name: "波形游标读数" })).not.toContainText("Δt");
});

test("车型速度环组织参数并只在确认后应用推荐波形", async ({ page }) => {
  await page.goto("/live/car-01");
  await page.getByLabel("车型配置", { exact: true }).selectOption("dicar-diff-drive");
  await page.getByRole("button", { name: "连接模拟器" }).click();
  await page.getByRole("button", { name: "速度环", exact: true }).click();

  await expect(page.getByText("实际", { exact: true })).toBeVisible();
  await expect(page.getByText("误差", { exact: true })).toBeVisible();
  await expect(page.getByLabel("速度环 Kp")).toBeVisible();
  await expect(page.getByText("5/8 通道", { exact: true })).toBeVisible();
  await expect(page.getByText(/设备清单未提供可写目标参数/)).toBeVisible();
  await expect(page.getByText("5/8 通道", { exact: true })).toBeVisible();

  await page.getByRole("button", { name: "应用 500 Hz 订阅" }).click();
  await expect(page.getByText("5/8 通道", { exact: true })).toBeVisible();
});

test("车型任务在窄窗口中仍保持核心入口可达", async ({ page }) => {
  await page.setViewportSize({ width: 640, height: 360 });
  await page.goto("/live/car-01");
  await page.getByLabel("车型配置", { exact: true }).selectOption("dicar-diff-drive");
  await page.getByRole("button", { name: "连接模拟器" }).click();
  const selector = page.getByLabel("车型配置", { exact: true });
  const speedTask = page.getByRole("button", { name: "速度环", exact: true });
  await selector.focus();
  await tabTo(page, speedTask);
  await page.keyboard.press("Enter");
  await tabTo(page, page.getByLabel("速度环 Kp"));
  await expect(page.getByLabel("速度环 Kp")).toBeFocused();
  await expect(page.getByRole("heading", { name: "实时波形" })).toBeVisible();
  await expect(page.getByRole("table")).toBeVisible();
  await tabTo(page, page.getByRole("region", { name: "实时波形交互区" }));
  await expect(page.getByRole("region", { name: "实时波形交互区" })).toBeFocused();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth)).toBe(true);
});

async function tabTo(page: Page, target: Locator, maxTabs = 48): Promise<void> {
  for (let index = 0; index < maxTabs; index += 1) {
    await page.keyboard.press("Tab");
    if (await target.evaluate((element) => element === document.activeElement).catch(() => false)) return;
  }
  throw new Error(`键盘 Tab 未在 ${maxTabs} 步内到达目标控件`);
}

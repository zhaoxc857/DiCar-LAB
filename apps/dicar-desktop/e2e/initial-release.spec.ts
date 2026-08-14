import { expect, test, type Locator, type Page } from "playwright/test";
import { readFile } from "node:fs/promises";

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
  await expect(page.getByLabel("目标速度")).toBeVisible();
  await expect(page.getByLabel("速度环 Kp")).toBeVisible();
  await expect(page.getByLabel("速度环 Ki")).toBeVisible();
  await expect(page.getByLabel("速度环 Kd")).toBeVisible();
  await expect(page.getByText("5/8 通道", { exact: true })).toBeVisible();
  await expect(page.getByText(/设备清单未提供可写目标参数/)).toHaveCount(0);

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

test("参数方案可保存、应用恢复 RAM 并在固化后生成记录", async ({ page }) => {
  await page.goto("/live/car-01");
  await page.getByRole("button", { name: "连接模拟器" }).click();
  await expect(page.getByRole("button", { name: "断开设备" })).toBeVisible();

  await page.getByLabel("速度环 Kp").fill("1.8");
  await page.getByRole("button", { name: "写入 RAM" }).click();
  await expect(page.getByText("1 项待固化")).toBeVisible();

  await page.getByRole("button", { name: "参数方案" }).click();
  await expect(page.getByRole("dialog", { name: "参数方案" })).toBeVisible();
  await page.getByLabel("方案名称").fill("直道方案");
  await page.getByRole("button", { name: "保存方案" }).click();
  await expect(page.getByText(/已保存「直道方案」/)).toBeVisible();
  await page.getByRole("button", { name: "关闭" }).click();

  await page.getByLabel("速度环 Kp").fill("2.4");
  await page.getByRole("button", { name: "写入 RAM" }).click();

  await page.getByRole("button", { name: "参数方案" }).click();
  await page.getByRole("button", { name: "应用", exact: true }).click();
  await expect(page.getByText(/1 项将写入 RAM/)).toBeVisible();
  await page.getByRole("button", { name: /写入 1 项到 RAM/ }).click();
  await expect(page.getByText(/已写入 1 项/)).toBeVisible();
  await page.getByRole("button", { name: "返回列表" }).click();
  await page.getByRole("button", { name: "关闭" }).click();
  await expect(page.getByLabel("速度环 Kp")).toHaveValue("1.8");

  await page.getByRole("button", { name: "审阅并固化" }).click();
  await page.getByRole("button", { name: "固化到 Flash" }).click();
  await expect(page.getByText("0 项待固化")).toBeVisible();
  await page.getByRole("button", { name: "参数方案" }).click();
  await expect(page.getByText(/固化记录 · Gen 1/)).toBeVisible();
});

test("Mock 波形可记录、因订阅变化封存、回放并下载 JSON 与 CSV", async ({ page }) => {
  await page.goto("/live/car-01");
  await page.getByRole("button", { name: "连接模拟器" }).click();
  await expect(page.getByRole("button", { name: "断开设备" })).toBeVisible();

  await page.getByRole("button", { name: "开始波形记录" }).click();
  await page.getByLabel("记录名称").fill("E2E 速度阶跃");
  await page.getByLabel("记录备注").fill("mock raw batches");
  await page.getByRole("button", { name: "确认开始" }).click();
  await expect(page.getByText(/正在记录 · E2E 速度阶跃/)).toBeVisible();
  await page.waitForTimeout(150);

  await page.getByRole("button", { name: "应用 500 Hz 订阅" }).click();
  await expect(page.getByText("遥测订阅变化，记录已自动保存")).toBeVisible();
  await page.getByRole("button", { name: "波形记录", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "波形记录库" })).toBeVisible();
  const recordingRow = page.getByTestId("recording-row").filter({ hasText: "E2E 速度阶跃" });
  await expect(recordingRow).toBeVisible();
  await expect(recordingRow).toContainText("订阅变化");

  await page.getByRole("button", { name: "回放 E2E 速度阶跃" }).click();
  await expect(page.getByRole("dialog", { name: "回放 · E2E 速度阶跃" })).toBeVisible();
  await expect(page.getByRole("img", { name: "回放波形" })).toBeVisible();
  await page.getByRole("button", { name: "播放回放" }).click();
  await page.getByRole("button", { name: "暂停回放" }).click();
  await page.getByRole("button", { name: "关闭波形回放" }).click();

  await page.getByRole("button", { name: "波形记录", exact: true }).click();
  const jsonDownloadPromise = page.waitForEvent("download");
  await page.getByRole("button", { name: "导出 JSON E2E 速度阶跃" }).click();
  const jsonDownload = await jsonDownloadPromise;
  expect(jsonDownload.suggestedFilename()).toMatch(/\.json$/);
  const jsonPath = await jsonDownload.path();
  if (jsonPath === null) throw new Error("JSON download path unavailable");
  const json = JSON.parse(await readFile(jsonPath, "utf8")) as {
    format: string;
    schemaVersion: number;
    metadata: { stopReason: string; stats: { pointCount: number } };
  };
  expect(json).toMatchObject({
    format: "dicar-telemetry-recording",
    schemaVersion: 1,
    metadata: { stopReason: "subscriptionChanged" },
  });
  expect(json.metadata.stats.pointCount).toBeGreaterThan(0);

  const csvDownloadPromise = page.waitForEvent("download");
  await page.getByRole("button", { name: "导出 CSV E2E 速度阶跃" }).click();
  const csvDownload = await csvDownloadPromise;
  expect(csvDownload.suggestedFilename()).toMatch(/\.csv$/);
  const csvPath = await csvDownload.path();
  if (csvPath === null) throw new Error("CSV download path unavailable");
  const csv = await readFile(csvPath, "utf8");
  expect(csv).toContain("batch_index,subscription_version,dropped_before,timestamp_us,sample_sequence");
});

async function tabTo(page: Page, target: Locator, maxTabs = 48): Promise<void> {
  for (let index = 0; index < maxTabs; index += 1) {
    await page.keyboard.press("Tab");
    if (await target.evaluate((element) => element === document.activeElement).catch(() => false)) return;
  }
  throw new Error(`键盘 Tab 未在 ${maxTabs} 步内到达目标控件`);
}

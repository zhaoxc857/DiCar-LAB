import { expect, test, type Locator, type Page } from "playwright/test";
import AxeBuilder from "@axe-core/playwright";
import { readFile } from "node:fs/promises";

test("B 菜单进入 A 工作台并完成写 RAM 与固化", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "工作区" })).toBeVisible();

  await page.getByRole("link", { name: "进入实时调试" }).click();
  await expect(page.getByRole("heading", { name: "实时调参与波形" })).toBeVisible();
  await connectSimulator(page);

  await page.getByLabel("速度环 Kp").fill("1.8");
  await page.getByRole("button", { name: "写入 RAM" }).click();
  await expect(page.getByText("1 项待固化")).toBeVisible();

  await page.getByRole("button", { name: "审阅并固化" }).click();
  await expect(page.getByRole("dialog", { name: "固化参数修改" })).toBeVisible();
  await page.getByRole("button", { name: "固化到 Flash" }).click();
  await expect(page.getByText("0 项待固化")).toHaveCount(0);
});

test("旧首页四入口兼容新工作台和参数方案历史链接", async ({ page }) => {
  await page.goto("/");
  const cards = [
    page.getByRole("link", { name: "进入实时调试" }),
    page.getByRole("link", { name: "打开波形记录" }),
    page.getByRole("link", { name: "管理参数方案" }),
    page.getByRole("link", { name: "查看链路诊断" }),
  ];
  for (const card of cards) await expect(card).toBeVisible();
  const desktopBoxes = await Promise.all(cards.map((card) => card.boundingBox()));
  expect(desktopBoxes[0]?.y).toBe(desktopBoxes[1]?.y);
  expect(desktopBoxes[2]?.y).toBe(desktopBoxes[3]?.y);
  expect(desktopBoxes[2]?.y ?? 0).toBeGreaterThan(desktopBoxes[0]?.y ?? 0);

  await page.setViewportSize({ width: 640, height: 720 });
  const narrowBoxes = await Promise.all(cards.map((card) => card.boundingBox()));
  expect(narrowBoxes[1]?.y ?? 0).toBeGreaterThan(narrowBoxes[0]?.y ?? 0);
  expect(narrowBoxes[2]?.y ?? 0).toBeGreaterThan(narrowBoxes[1]?.y ?? 0);
  expect(narrowBoxes[3]?.y ?? 0).toBeGreaterThan(narrowBoxes[2]?.y ?? 0);

  await cards[2].click();
  const dialog = page.getByRole("dialog", { name: "参数方案" });
  await expect(dialog).toBeVisible();
  await expect(page).toHaveURL(/\/live\?panel=snapshots$/);
  await dialog.getByRole("button", { name: "关闭" }).click();
  await expect(page).toHaveURL(/\/live$/);

  await page.goto("/parameter-sets");
  await expect(page.getByRole("dialog", { name: "参数方案" })).toBeVisible();
  await expect(page).toHaveURL(/\/live\?panel=snapshots$/);
});

test("Observer 只能查看波形，不能修改或固化", async ({ page }) => {
  await page.goto("/live");
  await connectSimulator(page);
  await page.getByLabel("演示身份").selectOption("observer");

  await expect(page.getByText("仅观察者不能修改参数")).toBeVisible();
  await expect(page.getByLabel("速度环 Kp")).toBeDisabled();
  await expect(page.getByRole("button", { name: "审阅并固化" })).toHaveCount(0);
  await expect(page.getByText("8/8 通道", { exact: true })).toBeVisible();
});

test("真实串口入口展示 HC-05 配对、传出 COM 和电平安全说明", async ({ page }) => {
  await page.goto("/");
  await openConnectionDrawer(page);
  await page.getByLabel("连接方式").selectOption("serial");
  await page.getByLabel("硬件类型").selectOption("hc05BluetoothSpp");
  await expect(page.getByRole("option", { name: "自动探测" })).toBeAttached();
  await page.getByRole("button", { name: "硬件指南" }).click();
  await expect(page.getByText(/Windows 蓝牙设置中完成配对/)).toBeVisible();
  await expect(page.getByText(/传出（Outgoing）COM 口/)).toBeVisible();
  await expect(page.getByText(/5V MCU 发往 HC-05 RX 时必须分压/)).toBeVisible();
});

test("波形 A/B 可纯键盘操作并在 200% 缩放下保留读数", async ({ page }) => {
  await page.goto("/live");
  await connectSimulator(page);
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
  await page.goto("/live");
  await selectVehicleProfile(page, "dicar-diff-drive");
  await connectSimulator(page);
  await page.getByRole("button", { name: "速度环", exact: true }).click();

  const workbench = page.getByTestId("workbench-layout");
  await expect(workbench.getByText("实际", { exact: true })).toBeVisible();
  await expect(workbench.getByText("误差", { exact: true })).toBeVisible();
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
  await page.goto("/live");
  await selectVehicleProfile(page, "dicar-diff-drive");
  await connectSimulator(page);
  const speedTask = page.getByRole("button", { name: "速度环", exact: true });
  await page.getByRole("button", { name: /已就绪，打开设备连接/ }).focus();
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
  await page.goto("/live");
  await connectSimulator(page);

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
  await expect(page.getByText("0 项待固化")).toHaveCount(0);
  await page.getByRole("button", { name: "参数方案" }).click();
  await expect(page.getByText(/固化记录 · Gen 1/)).toBeVisible();
});

test("Mock 波形可记录、因订阅变化封存、回放并下载 JSON 与 CSV", async ({ page }) => {
  await page.goto("/live");
  await connectSimulator(page);

  await page.getByRole("button", { name: "开始波形记录" }).click();
  await page.getByLabel("记录名称").fill("E2E 速度阶跃");
  await page.getByLabel("记录备注").fill("mock raw batches");
  await page.getByRole("button", { name: "确认开始" }).click();
  await expect(page.getByText(/正在记录 · E2E 速度阶跃/)).toBeVisible();
  await page.waitForTimeout(150);

  await page.getByRole("button", { name: "应用 500 Hz 订阅" }).click();
  await expect(page.getByText("遥测订阅变化，记录已自动保存")).toBeVisible();
  await page.getByRole("link", { name: "打开波形记录库" }).click();
  await expect(page.getByRole("heading", { name: "波形记录" })).toBeVisible();
  const recordingRow = page.getByTestId("recording-row").filter({ hasText: "E2E 速度阶跃" });
  await expect(recordingRow).toBeVisible();
  await expect(recordingRow).toContainText("订阅变化");

  await page.getByRole("button", { name: "回放 E2E 速度阶跃" }).click();
  await expect(page.getByRole("dialog", { name: "回放 · E2E 速度阶跃" })).toBeVisible();
  await expect(page.getByRole("img", { name: "回放波形" })).toBeVisible();
  await page.getByRole("button", { name: "播放回放" }).click();
  await page.getByRole("button", { name: "暂停回放" }).click();
  await page.getByRole("button", { name: "关闭波形回放" }).click();

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

test("标准与赛道模式共享状态且记录库是独立真实页面", async ({ page }) => {
  await page.goto("/live");
  await connectSimulator(page);
  await page.getByLabel("速度环 Kp").fill("1.8");
  await page.getByRole("button", { name: "赛道模式" }).click();
  await expect(page.getByTestId("workbench-layout")).toHaveAttribute("data-workbench-mode", "track");
  await expect(page.getByLabel("速度环 Kp")).toHaveValue("1.8");
  await page.getByRole("link", { name: "打开波形记录库" }).click();
  await expect(page.getByRole("heading", { name: "波形记录" })).toBeVisible();
  await expect(page.getByRole("button", { name: "导入记录 JSON" })).toBeAttached();
});

test("窄窗口导航和关键页面没有严重可访问性问题", async ({ page }) => {
  await page.setViewportSize({ width: 640, height: 720 });
  await page.goto("/");
  await page.getByRole("button", { name: "打开主导航" }).click();
  await expect(page.getByRole("link", { name: "波形记录" })).toBeVisible();
  const results = await new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa"]).analyze();
  expect(results.violations.filter((item) => item.impact === "critical" || item.impact === "serious")).toEqual([]);
});

async function openConnectionDrawer(page: Page): Promise<Locator> {
  await page.getByRole("button", { name: /打开设备连接/ }).click();
  const dialog = page.getByRole("dialog", { name: "设备连接" });
  await expect(dialog).toBeVisible();
  return dialog;
}

async function connectSimulator(page: Page): Promise<void> {
  const dialog = await openConnectionDrawer(page);
  await dialog.getByRole("button", { name: "连接模拟器" }).click();
  await expect(dialog.getByText("已就绪")).toBeVisible();
  await dialog.getByRole("button", { name: "关闭设备连接" }).click();
}

async function selectVehicleProfile(page: Page, profileId: string): Promise<void> {
  const dialog = await openConnectionDrawer(page);
  await dialog.getByRole("button", { name: "偏好" }).click();
  await dialog.getByLabel("车型配置", { exact: true }).selectOption(profileId);
  await dialog.getByRole("button", { name: "关闭设备连接" }).click();
}

async function tabTo(page: Page, target: Locator, maxTabs = 48): Promise<void> {
  for (let index = 0; index < maxTabs; index += 1) {
    await page.keyboard.press("Tab");
    if (await target.evaluate((element) => element === document.activeElement).catch(() => false)) return;
  }
  throw new Error(`键盘 Tab 未在 ${maxTabs} 步内到达目标控件`);
}

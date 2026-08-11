# DiCar Tune Public Documentation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the mixed-purpose README with a concise project portal and add separate, accurate user and developer guides for DiCar Tune 0.1.2.

**Architecture:** `README.md` is the single discovery page and links to two responsibility-focused documents. `docs/user-guide.md` owns installation, hardware operation, tuning, and troubleshooting; `docs/development.md` owns repository architecture, toolchains, validation, and packaging.

**Tech Stack:** GitHub-flavored Markdown, React 19/Vite 7 frontend, Tauri 2 desktop shell, Rust workspace, DCTP v1.

## Global Constraints

- Chinese is the primary language; commands, protocol names, and field names remain in English.
- Current release is exactly `0.1.2`.
- Windows release files are `release/DiCar-Tune-0.1.2-Windows-x64-Setup.exe` and `release/DiCar-Tune-0.1.2-Windows-x64-Portable.exe` relative to the main repository root.
- Wireless serial hardware provides transparent bytes only; the vehicle MCU must run DCTP firmware.
- Do not describe Web DCTP, cloud collaboration, AI tuning, or multi-vehicle concurrency as completed.
- Do not claim physical HC-05 or nanoUART-wl performance that was not verified with connected hardware.

---

### Task 1: Publish the README portal and focused guides

**Files:**
- Modify: `README.md`
- Create: `docs/user-guide.md`
- Create: `docs/development.md`

**Interfaces:**
- Consumes: the 0.1.2 release names, existing package scripts, Cargo workspace commands, serial hardware profiles, and telemetry budgets.
- Produces: stable relative documentation links `docs/user-guide.md` and `docs/development.md` used by the README.

- [ ] **Step 1: Run the documentation contract before creating the guides**

```powershell
$required = @('docs/user-guide.md', 'docs/development.md')
foreach ($path in $required) {
  if (-not (Test-Path -LiteralPath $path)) { throw "Missing public guide: $path" }
}
```

Expected: FAIL because both public guide files do not exist.

- [ ] **Step 2: Rewrite `README.md` as the project portal**

Use these exact top-level sections and keep detailed procedures in linked guides:

```markdown
# DiCar Tune
## 下载 Windows 0.1.2
## 5 分钟体验
## 已实现功能
## 硬件兼容性
## 文档
## 当前限制
## 开发与验证
## 项目状态
```

The download section links to the installer and portable executable. The quick start uses only the bundled simulator. The compatibility table lists nanoUART-wl, HC-05 Bluetooth SPP, generic COM, and Web Serial with honest support status and safe telemetry ceilings.

- [ ] **Step 3: Write `docs/user-guide.md`**

Use these exact sections:

```markdown
# DiCar Tune 用户手册
## 1. 安装与启动
## 2. 使用内置模拟器
## 3. 连接真实车辆前的准备
## 4. nanoUART-wl
## 5. HC-05 蓝牙串口
## 6. 实时调参工作流
## 7. 编码器参数
## 8. 遥测波形与链路上限
## 9. RAM、Flash 与断线状态
## 10. 常见问题
## 11. 当前限制
```

Include TX/RX crossover, common ground, HC-05 outgoing COM selection, the 3.3 V logic warning, all auto-probe orders, nanoUART-wl 8×500 at 460800 baud, HC-05 4×50 normally and 2×10 at 9600 baud, and the requirement for DCTP firmware on the MCU.

- [ ] **Step 4: Write `docs/development.md`**

Use these exact sections:

```markdown
# DiCar Tune 开发文档
## 1. 架构
## 2. 仓库结构
## 3. 开发环境
## 4. 安装依赖与运行 Web
## 5. 运行模拟器
## 6. 运行 Windows 桌面 App
## 7. 质量门禁
## 8. DCTP 黄金向量
## 9. Windows 打包
## 10. 修改硬件适配时的约束
## 11. 贡献建议
```

Document the existing root pnpm scripts, all-target Cargo format/Clippy/test commands, vector `--check`, Tauri NSIS command, MSVC requirement, and the responsibilities of `dctp-protocol`, `dctp-sim`, `dicar-app-core`, React, and the Tauri bridge.

- [ ] **Step 5: Validate files, links, versions, and prohibited completion claims**

```powershell
$required = @('README.md', 'docs/user-guide.md', 'docs/development.md')
foreach ($path in $required) {
  if (-not (Test-Path -LiteralPath $path)) { throw "Missing documentation: $path" }
}
$readme = Get-Content -Raw README.md
foreach ($link in @('docs/user-guide.md', 'docs/development.md')) {
  if (-not $readme.Contains($link)) { throw "README missing link: $link" }
  if (-not (Test-Path -LiteralPath $link)) { throw "Broken README link: $link" }
}
if (-not $readme.Contains('0.1.2')) { throw 'README release version is stale' }
rg -n "TBD|TODO|0\.1\.1" README.md docs/user-guide.md docs/development.md
git diff --check
```

Expected: the PowerShell contract and `git diff --check` pass; `rg` returns no matches.

- [ ] **Step 6: Commit the public documentation**

```powershell
git add README.md docs/user-guide.md docs/development.md
git commit -m "docs: publish user and developer guides"
```

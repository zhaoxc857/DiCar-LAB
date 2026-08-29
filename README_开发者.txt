DiCAR LAB · 开发者速查卡
（完整开发/贡献说明见 README.md 与 CONTRIBUTING.md）

发布结构：
  CAR_LAB\                    软件本体（入口 CAR_LAB\main.py）
  DiCAR_Launcher.py            小白启动器源码
  build_portable_windows.bat  Windows 便携版打包脚本
  dicar_lab.spec              PyInstaller 一目录配置
  logs\                       启动/错误日志

从源码运行（开发）：
  cd CAR_LAB
  python -m venv .venv
  .venv\Scripts\activate          （macOS/Linux: source .venv/bin/activate）
  pip install -r requirements.txt
  python main.py

运行测试：
  CAR_LAB\.venv\Scripts\python.exe -m unittest discover -s tests -v

Windows 打包桌面版：
  双击 build_portable_windows.bat
  生成 release\DiCAR-LAB-v1.7.0-Windows-x64.zip 与 SHA256SUMS.txt

发布给普通用户时只需：
  发布 ZIP 与 SHA256SUMS.txt；用户解压后运行 DiCAR LAB.exe。

新增车型 / 提交贡献：
  见 CONTRIBUTING.md（vehicles\<name>\config.yaml，加一个文件夹即一个车型）。

下一阶段：为固件烧录工作区接入按芯片/探针配置的可替换后端，在启用连续烧录前完成目标校验、写后验证、取消与失败恢复测试。当前版本不会执行烧录命令。

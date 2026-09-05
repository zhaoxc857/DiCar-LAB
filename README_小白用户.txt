DiCAR LAB · 小白用户速查卡
（完整说明见仓库主页 README.md）

怎么启动：
  1) 打开 https://github.com/zhaoxc857/DiCar-LAB/releases
  2) 下载 DiCAR-LAB-v1.10.0-Windows-x64.zip
  3) 完整解压后双击 DiCAR LAB.exe
  桌面版已包含运行环境，不用安装 Python，也不用运行安装器。

以后：
  还是双击解压目录里的 DiCAR LAB.exe 即可。

不用手动做这些：
  main.py / pip install / python 命令 / PyCharm 都不需要。

第一次想先玩：
  软件打开后，顶部「连接方式」选「仿真」→ 点「连接」，
  不接硬件也能体验示波器、PID 调参等全部功能。

启动失败怎么办：
  保留错误截图，并到项目 Issues 反馈：
  https://github.com/zhaoxc857/DiCar-LAB/issues

固件烧录：
  工具页「固件烧录」支持 STM32F1/F4 与 TI MSPM0G3507 无线烧录。
  每次烧录自动存入固件版本库，可写备注、随时回退到旧固件。
  使用前先按烧录页里的指引让车辆进入 bootloader 模式（BOOT0 跳线 / BSL 键）。

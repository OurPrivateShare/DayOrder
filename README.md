# DayOrder

DayOrder 是使用 Rust 与 Slint 构建的跨平台桌面任务规划器，面向 Linux 与 Windows。

## 功能

- 按年、月、日期浏览日历，并管理当天任务：新增、编辑、删除、完成状态。
- 按日期保存历史记录，统计年度任务总量、完成率与有记录的天数。
- 独立的今日任务浮窗：直接勾选任务、查看完成百分比、调整透明度、置顶、锁定拖动位置。
- 系统托盘菜单可重新打开规划中心、显示或隐藏今日浮窗，以及退出程序；关闭窗口不会退出。
- 每分钟检查本地日期，跨过午夜后自动切换到当天。

## 运行

需要 Rust stable 工具链。Debian/Ubuntu 构建需安装 `libfontconfig1-dev`；Fedora/RHEL 系列需安装 `fontconfig-devel`。X11/Wayland 桌面运行时依赖按所用发行版提供。

```sh
cargo run --release
```

Windows 可在 Windows 环境下执行同一命令，或使用 `cargo build --release` 生成可执行文件。Linux 的 Wayland 窗口管理器可能限制程序主动定位窗口；拖动、置顶和透明度仍取决于桌面合成器及其策略。

任务数据写入当前用户的数据目录下的 `DayOrder/tasks.json`（Linux 通常为 `~/.local/share/DayOrder/`；Windows 使用系统用户数据目录）。

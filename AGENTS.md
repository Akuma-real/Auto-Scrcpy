# Repository Guidelines

## 项目结构与模块组织
- `src/main.rs` 程序入口；`device_monitor.rs` 设备与 scrcpy 管理；`tui.rs` 终端界面；`single_instance.rs` 单实例。
- `Cargo.toml` 依赖与构建；`.github/workflows` CI 构建与发布；`README.md` 使用说明。
- 目标平台：Windows，发布包内置 `scrcpy/`（无需额外安装）。

## 构建、测试与开发命令
- 构建发布：`cargo build --release`（产物位于 `target\release\scrcpy-launcher.exe`）。
- 本地运行：`cargo run`（开发调试）。
- 代码格式化：`cargo fmt`；静态检查：`cargo clippy -- -D warnings`。
- 测试：`cargo test`（当前缺少测试，建议新增关键路径用例）。

## 代码风格与命名约定
- Rust 标准风格：4 空格缩进；`snake_case`（函数/变量），`CamelCase`（类型/结构体），模块名小写。
- 注释与文档中文优先，面向读者解释“为何如此做”。
- 提交前本地执行：`cargo fmt && cargo clippy`，确保零告警与一致格式。

## 测试指南
- 推荐使用内置单元测试（`#[cfg(test)]`），模块内 `mod tests { ... }`。
- 覆盖关键分支：设备检测、进程管理、TUI 状态渲染。
- 约定：用例命名清晰可读，断言可复现，避免对真实设备做强依赖。

## 提交与 Pull Request 规范
- 提交信息：动词开头、简洁明确，例如：`feat(tui): 优化日志滚动表现`。
- PR 要求：变更摘要、动机背景、截图/终端输出、关联 Issue、自测说明与影响评估。
- 变更最小化；不提交编译产物与私密文件（遵循 `.gitignore`）。

## 安全与配置提示
- 不泄露本地路径、令牌或设备信息；遵守 Windows 权限策略。
- 如需调整版本发布，请同步修改 `Cargo.toml` 并关注 CI 工作流结果。
- 避免破坏 CI 构建与发布流程（`.github/workflows/*.yml`）。

## CI/CD 概览
- `main` 分支推送触发构建与发布；版本由 `Cargo.toml` 控制。
- 发布产物包含 `scrcpy-launcher.exe` 与内置 `scrcpy/`，开箱即用。
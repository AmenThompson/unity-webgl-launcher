# Unity WebGL 启动器

适用于 Windows x64 的 Unity WebGL 本地播放器。选择 Unity 导出的 WebGL 发布目录，启动器会在本机启动 HTTP 服务，并在系统默认浏览器中打开场景。可以用它预览构建、演示项目或在多个本地项目之间切换。

[下载 Windows x64 版](https://github.com/AmenThompson/unity-webgl-launcher/releases/latest) · [提交问题](https://github.com/AmenThompson/unity-webgl-launcher/issues) · [MIT 许可证](LICENSE)

## 快速开始

1. 在 Releases 的 **Assets** 下载 Windows x64 EXE。运行需要 [Microsoft Edge WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/) 和支持 WebGL 的默认浏览器。
2. 双击 EXE 打开启动器。选择 **单项目**，再选择 Unity WebGL 的发布目录或其中的 `index.html`。
3. 点击 **启动并打开**，在浏览器中查看场景。可以在启动器内停止服务，关闭启动器也会结束本机服务。

请保留原始导出目录结构，包括项目实际使用的 `Build`、`TemplateData` 和 `StreamingAssets`。启动器只监听本机回环地址。场景若引用线上资源，仍会按其发布配置联网。

## 主要功能

| 功能 | 用法 |
| --- | --- |
| 多项目管理 | 选择发布总目录，搜索并切换项目。支持收藏、最近项目和默认发布目录。 |
| 压缩格式识别 | 自动识别常见的 Brotli、Gzip、未压缩及 Unity 解压回退构建。 |
| 多线程支持 | 对启用了 WebGL 线程的 Unity 构建打开该选项。跨源资源需满足浏览器的跨源隔离要求。 |
| 运行日志与自检 | 查看本地资源请求。内置 WebGL 自检可验证绘制、键盘、鼠标及全屏。 |

### 常见问题

- **场景空白或出现 404**：展开运行日志并检查浏览器控制台，确认导出目录完整。若模板使用 `/Build/` 这类根路径，请检查 Unity 构建模板是否支持相对路径。
- **多线程场景加载失败**：确认 Unity 项目启用了 WebGL 线程，再打开启动器的多线程选项。检查外部资源的跨源响应头。
- **下载校验**：下载同一版本的 `SHA256SUMS.txt`，在下载目录运行 `Get-FileHash -Algorithm SHA256 .\Unity-WebGL-Launcher_0.1.0_x64.exe`，比对 SHA-256。

## 从源码构建

应用使用 Tauri 2、Vue 3 和 Rust Axum。开发环境需要 Node.js 24、PowerShell 7、Rust 的 Windows MSVC x64 工具链、Visual Studio C++ Build Tools、Windows SDK 和 WebView2 Runtime。

```powershell
rustup target add x86_64-pc-windows-msvc
npm ci
npm run lint
npm run typecheck
npm test
npm run build
cargo test --locked --manifest-path src-tauri/Cargo.toml
npm run build-release
```

本地开发使用 `npm run tauri-dev`。发布脚本会在 `release/<版本>/` 生成便携 EXE 和 SHA-256 校验文件。首次发布及后续发版步骤见 [GitHub 新项目发布指南](<GitHub 新项目发布指南.md>)。

提交问题时，请附上应用版本、Unity 版本、WebGL 发布配置与必要的日志。欢迎通过 Pull Request 提交改进。

本项目采用 [MIT 许可证](LICENSE)。Unity 是其权利方的商标，本项目为独立工具。

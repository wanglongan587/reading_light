# 微光阅读 · Glow Reading

版本 0.1.5。一个 Windows 优先、本地保存书库、划线与日记的阅读应用。阅读功能可以离线使用；“此刻心情”通过 OpenAI API 根据本次输入生成回复，需要联网与 API 额度。Rust / GPUI Kit 负责原生界面，WebView 与 epub.js 负责 EPUB 书页。

本版新增全书搜索、持久化划线、阅读进度和阅读中快捷排版：可以从书架继续上次阅读，按章节和百分比查看进度，在任意章节中搜索并跳转到命中段落，把选中的句子留下划线并随时回到原文，也可以直接调整字号、行距和版心宽度。同时保留心情自由输入、本地日记、三套主题和原有隐私边界，并修复“减少动态”设置不影响书页动画、移出书架后缓存未清理的问题。

## 直接运行

解压 `dist/glow-reading-windows-x64.zip`（或运行 NSIS 安装包），双击 `glow-reading.exe`。Windows 需要 Microsoft Edge WebView2 Runtime；启动阅读失败时会显示说明，也可运行包内的 `Launch-Glow.ps1` 检查。官方安装页面：https://developer.microsoft.com/microsoft-edge/webview2/ 。发布流水线支持 Windows Authenticode 与 macOS Developer ID / Notarization 签名及 `latest.json` 更新清单。

首次打开会说明本地数据与演示回响的边界。内置原创短篇《在林间，慢慢来》，可完整阅读。选择“今天有些疲惫”等句子后点击“回响”，即可体验原文、出处和微光回复。界面以“安静阅读室”为统一语言：半透明纸张侧栏、原创手绘水彩风格插画、宋楷编辑标题与温柔护眼配色；内置「晴空旧书房」「夏风樟树下」「浅海晚风号」「山谷秘密花园」「海港烘焙屋」「雨夜灯笼巷」六套原创「微光空间」水彩主题，以及「素纸晨光」「静夜台灯」「星空棱镜」三套经典与光影主题，共九套可直接切换的阅读环境。

## 已实现的功能

- EPUB / TXT 文件选择与书架拖拽导入、摘要去重、封面与目录、滚动与翻页、CFI 进度恢复、书签、专注模式。
- 全文搜索：跨章节检索书中文字，显示章节和上下文摘要，点击结果直接定位；支持 `Ctrl+F`。
- 持久化划线：从选文回响留下划线，划线在重开书籍后恢复；可集中查看、回到原文或删除。
- 阅读进度与续读入口：显示当前章节和百分比，书架书卡显示进度；有历史位置时首页直接提供“继续上次阅读”。
- 阅读中快捷排版：字号、行距和版心宽度即时调整，跨书籍保留，并可一键恢复当前主题默认值。
- 兼容 XHTML DTD、命名空间、常见 HTML 章节和 URL 编码资源路径；脚本、远程资源和危险路径继续被拦截。
- 目录在正文首屏之前独立初始化；章节或字体资源等待过久时会换用备用正文入口，原生会话无响应时会自动重建一次阅读视图，避免永久停在“正在铺开这一页”。
- TXT 自动识别 UTF-8 / GB18030；阅读工具栏可重新选择编码。重新解码保留书籍身份、进度和书签。
- 阅读选文回响：45 条公有领域古典原文、6 类情绪标签、本地规则匹配、无匹配状态、出处链接、剧透折叠机制。阅读选文仍采用预写示例，明确标注。
- 此刻心情与防剧透 AI 共读：支持 OpenAI、DeepSeek、通义千问、硅基流动、本地离线 Ollama / LM Studio / llama.cpp 及自定义兼容接口。密钥仅存本机 `secrets.json`，绝不写入状态库或同步快照。
- 主动心情入口、本地私人日记、收藏、删除、清空与回到原书；界面按本地时区展示时间。
- 九套原创「微光空间」主题、主题编辑器、背景图片、字体、6 轨程序化自然声景混音台、预览取消、另存及 `.glowtheme` 包导入导出。环境音每次启动默认关闭。
- 合规听书与深夜电台：可插拔 `TtsProvider`（支持本机离线系统语音 + 用户自配置的 OpenAI 兼容神经语音接口与本地音频缓存），内置原创程序化治愈电台频道、本地私藏音频目录与自定义流 URL。
- 品牌侧栏、Lucide 矢量图标、明确的当前页状态、卡片悬停与按钮按压反馈；手绘场景在首页作为主视觉完整展开，书页内选文按钮使用同一套纸张、墨色与微光语义。
- 原生与书页统一主题；窄窗口下回响替换书页，模态窗口显示时隐藏 WebView，以规避当前 Windows 原生子窗口覆盖问题。

## 从源码运行

Windows：Rust MSVC、Visual Studio C++ Build Tools / Windows SDK、CMake、Node.js 22+、WebView2。本机验证使用 Rust 1.96.1。

```powershell
cd web
npm ci
npm run build
cd ..
cargo run -p glow-reading --locked
```

书页资源在编译时嵌入程序，因此修改 `web/src` 后必须重新构建网页和 Rust。`web/dist` 是可审查的生成资源，运行包不依赖 Node.js 或开发服务器。

macOS 基线为 macOS 15+ 与 Xcode Command Line Tools；Linux 基线为 Ubuntu 24.04、X11 / XWayland、WebKitGTK 4.1、Vulkan 驱动。依赖安装项见 `.github/workflows/ci.yml`。当前只完成 Windows 验证，其他平台的 CI 配置尚未在远程运行，不能据此声称已验收。

## 测试与打包

```powershell
cargo test -p glow-core --locked
cargo test -p glow-reading --features ui-test --locked
cargo run -p glow-core --example fixtures -- target/fixtures
cd web
npx playwright install chromium
npm test
cd ..
cargo build -p glow-reading --features ui-test --locked
powershell -ExecutionPolicy Bypass -File scripts/smoke-windows.ps1
powershell -ExecutionPolicy Bypass -File scripts/package-windows.ps1
```

Windows 已安装 Edge 时，可设置 `$env:PLAYWRIGHT_CHANNEL='msedge'` 跳过下载测试浏览器。原生烟雾测试使用隔离的 `target/` 数据，不触碰个人书库；其中的心情步骤会用虚构文字调用真实 API，需要本机密钥与额度，并可能计费。设置 `GLOW_SMOKE_FAKE_MOOD=1` 时只使用本地模拟回复验证界面与保存链路，真实 HTTP 请求和错误解析仍由 Rust 单元测试覆盖。Rust 单元和原生交互测试使用本地模拟，不调用真实 API。启用 `ui-test` 时烟雾测试同时输出 GPU 原生界面渲染图，不截图其他应用。

`scripts/package-windows.ps1` 生成运行 ZIP 和 SHA-256 校验文件。运行包需从新目录解压测试，不能只测试 `target/release`。

## 数据与结构

### 心情服务配置

密钥仅从本机 `OPENAI_API_KEY` 或 `.env.local` 读取，不写入 `state.json`、日志或发布 ZIP。开发运行从当前工作目录读取 `.env.local`；也可用 `GLOW_ENV_FILE` 指定已有文件。安装脚本在检测到项目 `.env.local` 时，仅在安装目录生成 `openai-config.json` 指向该文件，不复制密钥。移动该文件后应更新配置中的 `env_file`。

默认模型为 `gpt-4.1-mini-2025-04-14`，单次请求最长等待 60 秒，无自动重试。停止等待会丢弃晚到结果，已发出的请求仍可能计费。生成成功但保存失败时保留回答，再次提交相同文字与标签会只重试本机保存。API 余额与项目权限由 OpenAI 平台管理；没有密钥时阅读与本地日记仍可用。

`store: false` 不代表 OpenAI 不保留任何数据，服务端数据处理以 OpenAI API 数据政策为准：https://developers.openai.com/api/docs/guides/your-data 。

- `crates/core`：电子书解析与清理、持久化、主题包、回响语料、桥接协议及单元测试。
- `crates/desktop`：GPUI 原生页面、WebView 生命周期、受限资源协议、音频和原生交互测试。
- `web`：TypeScript 阅读面、epub.js、选文按钮、位置与主题同步、Playwright 测试。
- `assets`：原创短篇与场景插图、引用数据及来源说明。

数据位于系统本地应用数据目录的 `GlowReading` 项目目录；可用 `GLOW_DATA_DIR` 指定独立目录。核心状态以本地 SQLite 数据库 `glow.db`（WAL 模式 + FTS5 个人记忆全文检索 + 阅读会话历史）持久化，首次升级会自动从旧版 `state.json` 无损迁移并保留备份与可读快照；`secrets.json` 单独存放本机密钥；`books` 保存净化后的 EPUB 与 TXT 副本；`themes` 保存主题素材；`tts_cache` 缓存分段语音；`webview` 保存 WebView2 运行缓存，不进入发布包。原文件不被修改或删除。备份时关闭应用并复制整个数据目录。

状态变更通过 SQLite 事务与原子文件替换写入。数据损坏时停止加载并保留原文件，不以空状态覆盖。同一数据目录使用进程文件锁，第二个进程会提示先关闭已打开的窗口，避免并发覆盖日记。演进路线图见 [docs/ROADMAP.md](docs/ROADMAP.md)。

## 兼容边界

- 只支持无加密的可重排版 EPUB 和 TXT；PDF、MOBI、AZW3、固定版式、DRM、竖排排版与媒体叠加未验收。
- EPUB 需要可解析的 HTML/XHTML 章节。脚本、表单、嵌入对象及 SVG 内容会移除；远程 CSS 资源会移除，本地排版和图片样式尽量保留。远程清单资源拒绝导入。
- 单个 EPUB 与解压总量最多 100 MB，单项解压最多 32 MB；TXT 最多 20 MB，主题上传素材最多 25 MB。大量书库的虚拟化与复杂批注不在本轮范围内。
- 字体、背景及环境音由用户选择本地文件；分享主题包前需确保素材允许分享。
- 日记未加密；无云同步、无应用账户。心情 AI 回复用于日常文字陪伴，不进行医疗诊断。退出后停止播放环境音。
- 100% / 125% / 150% 系统缩放需要分别实机验收，自动化书页尺寸测试不等同于全部 DPI 验收。3—5 位目标用户试用尚待安排。

## 框架依据

- https://gpui-kit.com/docs/installation/
- https://gpui-kit.com/docs/webview/
- https://gpui-kit.com/docs/packaging/
- https://github.com/futurepress/epub.js

GPUI Kit 和匹配的 WebView 锁定为 0.9.0；`Cargo.lock` 固定间接依赖。epub.js 0.3.93 的旧 XML 解析依赖通过 npm override 固定到 `@xmldom/xmldom 0.9.12`，并有真实 EPUB 回归测试。

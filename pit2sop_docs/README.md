# Pit2SOP 说明文档

> 工作名：**Pit2SOP**  
> 定位：**把日常踩坑记录转成个人 SOP，并在你再次进入相似工作场景前提醒你。**

---

## 1. 项目目标

Pit2SOP 不是传统笔记软件，也不是单纯的语音转文字工具。它的核心目标是建立一个个人化的防错系统：

```text
随手记录一个坑
→ AI 提炼根因、修复方式、防错规则
→ 自动生成或更新 SOP
→ 当桌面端检测到相似工作场景时提醒执行 SOP
→ 执行后继续沉淀经验
```

项目优先面向个人自用，但版本边界必须收紧。当前 V0.1 只封 CLI 本地闭环；桌面壳、手机端、通知、Git hook 都是后续阶段。

长期设计重点是：

1. **低摩擦输入**：手机和电脑都能快速记录。
2. **桌面端主控**：电脑 Agent 负责分析、索引、写入 Obsidian、触发提醒。
3. **Obsidian 本地知识库**：Markdown 作为长期可迁移数据源。
4. **自动化场景触发**：Git、文件变化、浏览器、CLI、日历、手动输入等都可以成为触发源。
5. **AI 生成 SOP**：每个坑必须转化为可执行 checklist 或更新已有 SOP。

---

## 2. 核心产品形态

```text
手机端 = 输入遥控器
桌面端 = 主脑 Agent
Obsidian = 长期知识库
SQLite / 向量索引 = 本地缓存
AI = 分析和生成引擎
```

### 2.1 手机端职责

手机端只做捕捉，不做复杂知识管理：

- 语音输入
- 文本输入
- 图片 / 截图输入
- 分享菜单输入
- 将输入发送给电脑端
- 显示同步状态

### 2.2 桌面端职责

桌面端是系统主脑：

- 接收手机输入
- 提供桌面语音 / 文本输入
- 接收 CLI、Git hook、浏览器扩展、IDE 插件等信号
- AI 转写、分类、结构化、生成 SOP
- 写入 Obsidian Vault
- 建立 SQLite / 向量索引
- 触发桌面通知
- 打开对应 Obsidian 笔记

### 2.3 Obsidian 职责

Obsidian 不作为计算引擎，而作为长期知识库：

- 保存原始坑点
- 保存 SOP 文档
- 保存场景规则
- 保存复盘记录
- 支持人工编辑
- 支持 Git / Obsidian Sync / 云盘同步

---

## 3. 一句话流程

```text
你在手机或电脑上记录一个坑，电脑 Agent 自动把它写进 Obsidian，并生成/更新对应 SOP；当你下次在电脑上进入类似工作场景时，Agent 弹出提醒。
```

---

## 4. 典型使用流程

### 4.1 手机记录坑

你在外面用手机录音：

```text
记录一个坑：今天客户交付时忘了附测试账号，导致对方无法验收。下次交付前要检查测试账号、权限、测试数据和说明文档。
```

手机端生成 `CaptureEvent`，发送到电脑端。电脑端处理后：

1. 保存原始录音到 Obsidian 附件目录。
2. 转写语音。
3. AI 提炼坑点。
4. 创建 Pit 笔记。
5. 更新 `SOP - 客户交付前检查.md`。
6. 更新本地索引。

最终新增 checklist：

```markdown
- [ ] 是否附上测试账号
- [ ] 测试账号权限是否正确
- [ ] 是否准备测试数据
- [ ] 是否提供验收说明文档
```

### 4.2 桌面场景触发

你在电脑上执行：

```bash
git checkout -b release/2.5.0
```

Git hook 或 Desktop Agent 识别到 `release` 场景，弹出通知：

```text
检测到你进入发布流程。
建议执行：SOP - iOS 发布前检查
历史相关坑点：CI secret 未更新、证书过期、审核账号不可用。
```

点击通知后打开 Obsidian 中的 SOP。

---

## 5. MVP 范围

第一版只做闭环，不做复杂生态。

### V0.1 必须实现：CLI-only 本地闭环

| 模块 | 功能 |
|---|---|
| CLI | `sop pit`、`sop check`、`sop search`、`sop status` |
| Pending Patch | `sop pending`、`sop apply-patch`、`sop reject-patch` |
| Obsidian | 自动写入 Pit / SOP / pending patch Markdown |
| AI | 结构化 Pit，决定创建、更新、待确认或人工 review |
| 索引 | SQLite 存储处理状态和可重建搜索缓存 |
| 容错 | 手写 SOP 兼容、坏 frontmatter 不阻断提醒、失败状态可追踪 |

### V0.2 建议实现：Tauri 桌面壳

| 模块 | 功能 |
|---|---|
| 桌面 UI | 记录坑、doing、搜索、Pending Patches、设置 |
| 托盘 / 菜单栏 | 打开输入框、打开 Vault、退出 |
| 设置 | Vault path、AI provider、API key |
| Core 复用 | Tauri command 直接调用 `pit2sop-core` |

### V0.3 建议实现：桌面 Agent 与提醒

| 模块 | 功能 |
|---|---|
| Git hook | release 分支、tag、push、migration 文件变更触发 |
| 文件监听 | 监听 migrations、fastlane、CI workflow、env 配置 |
| 通知 | 桌面通知打开 SOP |
| 执行记录 | 忽略、稍后提醒、完成状态 |

### V0.4 建议实现：手机输入与外部信号

| 模块 | 功能 |
|---|---|
| 手机端 | 文本 / 语音 / 分享输入、本地队列、扫码配对 |
| 桌面接收 | LAN API、pairing token、状态查询 |
| 浏览器扩展 | 当前 URL / title / 选中文本发送给 Agent |
| 云中转 | 电脑不在线时手机先上传，电脑上线后拉取 |

---

## 6. 推荐技术栈

### 6.1 桌面端

优先推荐：

```text
Tauri + Rust Core + TypeScript UI
```

原因：

- 轻量
- 跨平台 Mac / Windows
- 适合常驻 Agent
- Rust 适合文件监听、HTTP 服务、SQLite、系统集成
- 前端可用 React / Vue / Svelte

备选：

```text
Electron + Node.js
```

优点是开发速度快、生态成熟，缺点是体积和资源占用更高。

### 6.2 手机端

第一阶段可以只做 iOS：

```text
SwiftUI + AVFoundation + URLSession
```

后续如果需要 Android：

```text
Flutter / React Native / Kotlin Multiplatform
```

如果确定长期要 iOS + Android，Flutter 或 React Native 会更省开发成本。

### 6.3 本地数据

```text
Obsidian Vault = source of truth
SQLite = 本地缓存 / 状态 / 索引
Vector Index = 语义搜索缓存，可重建
```

推荐 SQLite 表：

- `capture_events`
- `pits`
- `sops`
- `scenes`
- `trigger_events`
- `sop_executions`
- `file_index`
- `embeddings`

---

## 7. Obsidian Vault 目录结构

推荐创建单独 Vault：

```text
Pit2SOP/
├── 00_Inbox/
│   ├── Mobile Captures/
│   ├── Desktop Captures/
│   ├── Raw Logs/
│   └── Unprocessed/
│
├── 01_Pits/
│   ├── 2026/
│   └── Archive/
│
├── 02_SOPs/
│   ├── Development/
│   ├── Release/
│   ├── Client Delivery/
│   └── Personal Workflow/
│
├── 03_Scenes/
│   ├── iOS 发布.md
│   ├── 数据库迁移.md
│   ├── 客户交付.md
│   └── Code Review.md
│
├── 04_Reviews/
│   ├── Weekly/
│   ├── Monthly/
│   └── Executions/
│
├── 90_Attachments/
│   ├── Audio/
│   ├── Images/
│   ├── Screenshots/
│   └── Logs/
│
└── 99_System/
    ├── Templates/
    ├── Indexes/
    └── Agent Config.md
```

---

## 8. 三类核心文档

### 8.1 Pit：一次踩坑记录

用于记录一次具体问题，包括症状、根因、修复方式、防错规则、关联 SOP。

### 8.2 SOP：可执行流程

用于保存 checklist，执行时可被通知、搜索、打开。

### 8.3 Scene：触发场景

用于描述什么时候提醒你执行某个 SOP。例如：

- iOS 发布
- 数据库迁移
- 客户交付
- Code Review
- 支付链路变更
- CI/CD 修改

---

## 9. 关键设计原则

### 9.1 Obsidian 是主数据源

Markdown 文件是 source of truth。SQLite、向量索引、AI 缓存都可以重建。

### 9.2 Agent 不应该覆盖人工编辑

Agent 更新 SOP 时，只能修改受控区域，例如：

```markdown
<!-- pit2sop:start:auto-items -->
- [ ] 检查 CI secret 是否更新
<!-- pit2sop:end:auto-items -->
```

手写内容不能被自动覆盖。

### 9.3 手机端只做输入

手机端不负责完整 SOP 管理，避免复杂度失控。

### 9.4 桌面端负责提醒

真正的工作场景大多发生在电脑上，提醒应由桌面 Agent 触发。

### 9.5 每个坑必须有归宿

每条 Pit 处理完成后，必须进入以下状态之一：

```text
已关联已有 SOP
已创建新 SOP
暂不适合生成 SOP
需要人工确认
```

---

## 10. 后续开发顺序

建议按以下顺序实现：

```text
1. V0.1：CLI 本地闭环
2. V0.2：Tauri 桌面壳
3. V0.3：桌面 Agent、通知、Git hook、文件监听
4. V0.4：手机输入、浏览器扩展、云中转
5. V0.5：Obsidian 插件、周报复盘、SOP 执行历史
```

---

## 11. 项目最终形态

Pit2SOP 最终应该成为一个个人工作防错系统：

```text
记录过去的坑
理解当前的工作场景
提醒未来的风险
维护长期可复用的个人 SOP
```

它的价值不是“记下来”，而是：

> **在你再次进入类似场景之前，把过去踩过的坑推到你眼前。**

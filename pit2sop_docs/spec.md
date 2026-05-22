# Pit2SOP Product & Engineering Spec

> 版本：0.1  
> 状态：Draft  
> 目标平台：Mac / Windows 桌面端 + 手机输入端  
> 主存储：Obsidian Markdown Vault

---

## 1. 产品定义

Pit2SOP 是一个个人 SOP Agent。用户通过手机或电脑快速记录工作中遇到的坑，系统用 AI 将记录转化为结构化 Pit，并自动生成或更新个人 SOP。当用户在电脑上再次进入类似工作场景时，系统主动提醒执行对应 SOP。

### 1.1 产品目标

```text
记录一次坑 → 生成一条防错规则 → 更新一份 SOP → 下次做事前提醒
```

### 1.2 非目标

当前版本不追求：

- 多人协作
- 企业权限系统
- 完整项目管理
- 替代 Obsidian
- 替代 Todo / Calendar
- 复杂云端知识库

---

## 2. 用户画像

### 2.1 Primary User

个人开发者 / 技术负责人 / 独立开发者。

特征：

- 经常在开发、发布、交付、运维中踩坑
- 有使用 Obsidian / Markdown / Git 的习惯
- 主要工作发生在电脑上
- 希望低摩擦记录和自动提醒
- 不需要复杂隐私分级，但不能丢数据

---

## 3. 核心概念

## 3.1 CaptureEvent

任意输入源产生的原始事件。

来源包括：

- 手机语音
- 手机文本
- 手机分享菜单
- 桌面语音
- 桌面文本
- CLI
- Git hook
- 文件监听
- 浏览器扩展

## 3.2 Pit

一次具体踩坑记录。

字段：

- 标题
- 发生场景
- 表面症状
- 根因
- 修复方式
- 防错规则
- 风险等级
- 复发概率
- 关联 SOP

## 3.3 SOP

可执行的 checklist 文档。

字段：

- 名称
- 适用场景
- 触发关键词
- 检查清单
- 历史坑点
- 版本
- 状态

## 3.4 Scene

触发场景，用于判断什么时候提醒用户。

示例：

- iOS 发布
- 数据库迁移
- 客户交付
- Code Review
- CI/CD 修改
- 支付链路变更

## 3.5 TriggerEvent

桌面端检测到的工作场景信号。

来源包括：

- Git 分支
- Git tag
- 文件变化
- 当前浏览器页面
- CLI 输入
- 日历事件
- 提醒事项

---

## 4. 功能需求

## 4.1 手机端输入

### FR-MOB-001：语音输入

用户可以在手机端按住录音或点击开始录音。

验收标准：

- 可以录制不少于 10 分钟音频。
- 录音结束后生成 CaptureEvent。
- 音频文件进入本地待同步队列。
- 如果电脑在线，自动发送。
- 如果电脑不在线，保留在本地队列。

### FR-MOB-002：文本输入

用户可以快速输入文字记录。

验收标准：

- 输入完成后 1 次点击即可发送。
- 支持添加手动标签。
- 支持查看发送状态。

### FR-MOB-003：分享菜单输入

用户可以从其他 App 分享文本、URL 或图片到手机端。

验收标准：

- 支持接收纯文本。
- 支持接收 URL。
- 支持接收图片。
- 分享内容被包装为 CaptureEvent。

### FR-MOB-004：本地待同步队列

手机端必须保证输入不丢失。

状态机：

```text
created → queued → sending → delivered → processed
                     ↓
                   failed → retrying
```

验收标准：

- App 退出后队列仍存在。
- 网络失败后自动重试。
- 用户可以手动重试。
- 用户可以删除未发送事件。

### FR-MOB-005：电脑配对

用户可以通过桌面端二维码配对手机。

验收标准：

- 桌面端展示二维码。
- 手机扫描后保存 desktop_id、endpoint、pairing_token。
- 后续请求携带 pairing_token。
- 支持重新配对。

---

## 4.2 桌面端输入

### FR-DESK-001：全局快捷输入

用户可以通过全局快捷键唤起输入框。

默认快捷键：

```text
Mac: Cmd + Shift + P
Windows: Ctrl + Shift + P
```

输入模式：

- 记录一个坑
- 我要做一件事
- 搜索 SOP

验收标准：

- 快捷键可配置。
- 输入后生成 CaptureEvent 或 TriggerEvent。
- 结果可以直接打开对应 Obsidian 文件。

### FR-DESK-002：托盘 / 菜单栏入口

桌面端应常驻系统托盘或菜单栏。

菜单项：

- 记录一个坑
- 我要做一件事
- 搜索 SOP
- 今日风险
- 打开 Obsidian Vault
- 设置

### FR-DESK-003：CLI 输入

提供 `sop` CLI。

命令：

```bash
sop pit "今天 CI secret 忘记更新，导致 production 请求失败"
sop doing "我要上线 2.5.0"
sop check release
sop search "上次证书问题怎么解决"
sop status
```

验收标准：

- CLI 可以连接本地 Agent。
- Agent 未启动时给出明确提示。
- CLI 输出可读结果。

---

## 4.3 手机到电脑传输

### FR-SYNC-001：LAN Direct

桌面端启动本地接收服务。

默认端口：

```text
8765
```

接口：

```http
POST /v1/captures
POST /v1/captures/{id}/attachments
GET /v1/captures/{id}/status
```

验收标准：

- 手机和电脑同一局域网时可以直传。
- 请求必须包含 pairing_token。
- CaptureEvent ID 幂等。
- 重复发送不会重复创建 Pit。

### FR-SYNC-002：Cloud Relay fallback

当局域网直连失败时，手机可上传到云中转。

验收标准：

- 云中转只做临时存储。
- 电脑 Agent 上线后拉取未处理事件。
- 拉取成功后标记 delivered。
- 支持附件上传。

---

## 4.4 AI 处理

### FR-AI-001：音频转写

音频 CaptureEvent 需要转写成文本。

验收标准：

- 转写文本保存在 Pit 或 Inbox Markdown 中。
- 原始音频保存在 `90_Attachments/Audio/`。
- 转写失败时事件进入 failed 状态，可重试。

### FR-AI-002：输入分类

系统需要将输入分为：

| 类型 | 说明 |
|---|---|
| pit | 踩坑记录 |
| doing | 即将做某件事 |
| note | 普通笔记 |
| log | 错误日志 |
| sop_request | 明确要求生成 SOP |

验收标准：

- 分类结果写入 SQLite。
- 低置信度结果需要人工确认。

### FR-AI-003：Pit 结构化

对于 pit 类型输入，AI 输出：

```json
{
  "pit_title": "string",
  "scenario": "string",
  "symptom": "string",
  "root_cause": "string",
  "fix": "string",
  "prevention_rule": "string",
  "sop_candidate": "string",
  "trigger_keywords": ["string"],
  "risk_level": "low|medium|high",
  "recurrence_probability": "low|medium|high"
}
```

验收标准：

- 生成 Pit Markdown。
- 写入 YAML frontmatter。
- 关联 SOP 或创建 SOP 草稿。

### FR-AI-004：SOP 生成 / 更新

系统必须把 Pit 转成 SOP checklist。

规则：

- 如果匹配已有 SOP，则生成 patch。
- 如果没有匹配 SOP，则创建 SOP draft。
- 自动 patch 只能修改 marker 区块。
- 重大变更进入待确认。

验收标准：

- SOP 文件可读。
- checklist 可执行。
- 更新记录可追溯。

---

## 4.5 Obsidian 存储

### FR-OBS-001：选择 Vault 路径

用户可以在桌面端设置 Obsidian Vault 路径。

验收标准：

- 路径不存在时提示错误。
- 初始化时自动创建目录结构。
- 支持重新选择 Vault。

### FR-OBS-002：写入 Pit 文件

Pit 文件路径规则：

```text
01_Pits/{year}/{date} {sanitized-title}.md
```

示例：

```text
01_Pits/2026/2026-05-22 CI secret 未更新导致发布失败.md
```

### FR-OBS-003：写入 SOP 文件

SOP 文件路径规则：

```text
02_SOPs/{category}/SOP - {name}.md
```

示例：

```text
02_SOPs/Release/SOP - iOS 发布前检查.md
```

### FR-OBS-004：写入 Scene 文件

Scene 文件路径规则：

```text
03_Scenes/{scene-name}.md
```

### FR-OBS-005：安全写文件

写文件流程：

```text
read → hash check → patch → write temp → atomic rename → re-index
```

验收标准：

- 不覆盖人工修改。
- 写失败时不破坏原文件。
- 支持重复处理幂等。

---

## 4.6 场景触发与提醒

### FR-TRG-001：手动 doing 触发

用户输入：

```text
我要上线 2.5.0
```

系统返回：

- 检测场景
- 匹配 SOP
- 关联历史坑点
- 风险等级

### FR-TRG-002：Git hook 触发

支持以下触发：

- 创建 release 分支
- checkout release 分支
- push main/master
- 创建 tag
- 修改 migration 文件

验收标准：

- Git hook 可以安装到指定 repo。
- 触发时发送 TriggerEvent 到 Agent。
- Agent 返回建议 SOP。

### FR-TRG-003：文件监听触发

默认监听目录 / 文件模式：

```text
**/db/migrations/**
**/.github/workflows/**
**/fastlane/**
**/.env*
**/Dockerfile
**/docker-compose*.yml
**/k8s/**
```

验收标准：

- 文件变化不会高频刷屏。
- 需要 debounce。
- 同一场景短时间内只提醒一次。

### FR-TRG-004：浏览器扩展触发

浏览器扩展发送：

- 当前 URL
- 页面 title
- 选中文本

验收标准：

- 支持手动发送当前页面。
- 后续可支持自动规则。

### FR-TRG-005：通知提醒

桌面通知需要支持：

- 打开 SOP
- 稍后提醒
- 忽略本次

验收标准：

- 点击通知可以打开 Obsidian 对应文件。
- 忽略状态写入 SQLite。
- 一段时间内不重复提醒同一事件。

---

## 5. 非功能需求

## 5.1 性能

- 桌面 Agent 空闲内存目标：小于 200MB。
- 本地搜索响应：小于 500ms。
- 普通文本 Pit 处理：小于 30s，取决于 AI 服务。
- 文件监听事件 debounce：默认 3s。

## 5.2 可靠性

- 手机输入不能因为网络失败丢失。
- Obsidian 写入失败不能破坏原文件。
- SQLite 可从 Obsidian Vault 重建。
- CaptureEvent 必须幂等。

## 5.3 可迁移性

- 所有核心知识必须是 Markdown。
- YAML frontmatter 应保持稳定。
- SQLite 不作为唯一数据源。
- AI 输出需要保存原文和提炼结果，避免无法复查。

## 5.4 安全最低要求

虽然是个人自用，仍需满足：

- 配对 token
- 本地 API 鉴权
- 附件大小限制
- Cloud Relay token 鉴权
- 不接受未配对设备请求

---

## 6. 数据模型

## 6.1 CaptureEvent

```ts
export type CaptureSourceType =
  | "mobile_voice"
  | "mobile_text"
  | "mobile_share"
  | "desktop_voice"
  | "desktop_text"
  | "cli"
  | "git"
  | "file"
  | "browser";

export type CaptureStatus =
  | "created"
  | "queued"
  | "sending"
  | "delivered"
  | "processing"
  | "processed"
  | "failed";

export interface CaptureEvent {
  id: string;
  sourceDevice: string;
  sourceType: CaptureSourceType;
  createdAt: string;
  timezone: string;
  rawText?: string;
  attachments?: Attachment[];
  context?: Record<string, unknown>;
  status: CaptureStatus;
}

export interface Attachment {
  id: string;
  type: "audio" | "image" | "log" | "file";
  filename: string;
  mimeType: string;
  sizeBytes: number;
  localPath?: string;
}
```

## 6.2 Pit

```ts
export interface Pit {
  id: string;
  title: string;
  createdAt: string;
  sourceCaptureId: string;
  scenario: string;
  symptom: string;
  rootCause: string;
  fix: string;
  preventionRule: string;
  riskLevel: "low" | "medium" | "high";
  recurrenceProbability: "low" | "medium" | "high";
  sopId?: string;
  tags: string[];
  obsidianPath: string;
}
```

## 6.3 SOP

```ts
export interface SOP {
  id: string;
  title: string;
  version: number;
  status: "draft" | "active" | "archived";
  riskLevel: "low" | "medium" | "high";
  scenarios: string[];
  triggers: string[];
  checklist: ChecklistSection[];
  relatedPitIds: string[];
  obsidianPath: string;
}

export interface ChecklistSection {
  title: string;
  items: ChecklistItem[];
}

export interface ChecklistItem {
  id: string;
  text: string;
  sourcePitId?: string;
  status?: "open" | "done" | "skipped";
}
```

## 6.4 Scene

```ts
export interface Scene {
  id: string;
  name: string;
  riskLevel: "low" | "medium" | "high";
  triggerKeywords: string[];
  triggerSources: TriggerSource[];
  matchedSopIds: string[];
  obsidianPath: string;
}

export type TriggerSource =
  | "manual"
  | "git"
  | "file"
  | "browser"
  | "calendar"
  | "reminder"
  | "cli";
```

## 6.5 TriggerEvent

```ts
export interface TriggerEvent {
  id: string;
  source: TriggerSource;
  createdAt: string;
  payload: Record<string, unknown>;
  detectedScene?: string;
  matchedSopIds?: string[];
  confidence?: number;
  action?: "notified" | "ignored" | "snoozed" | "opened";
}
```

---

## 7. API Spec

## 7.1 Local Agent API

Base URL:

```text
http://127.0.0.1:8765
http://<lan-ip>:8765
```

All non-localhost requests require header:

```http
Authorization: Bearer <pairing_token>
```

### POST /v1/captures

Create or upsert a CaptureEvent.

Request:

```json
{
  "id": "cap_01JX9ZP3R4K7",
  "source_device": "iPhone 15",
  "source_type": "mobile_text",
  "created_at": "2026-05-22T15:30:00+09:00",
  "timezone": "Asia/Tokyo",
  "raw_text": "今天上线又漏了 CI secret，导致 production API 请求失败。",
  "context": {
    "manual_tags": ["release", "ci"]
  }
}
```

Response:

```json
{
  "ok": true,
  "capture_id": "cap_01JX9ZP3R4K7",
  "status": "delivered"
}
```

### POST /v1/captures/{id}/attachments

Upload attachment for a capture.

Request:

```http
Content-Type: multipart/form-data
```

Response:

```json
{
  "ok": true,
  "attachment_id": "att_01JX9ZRX2B",
  "status": "uploaded"
}
```

### GET /v1/captures/{id}/status

Response:

```json
{
  "capture_id": "cap_01JX9ZP3R4K7",
  "status": "processed",
  "created_pit_path": "01_Pits/2026/2026-05-22 CI secret 未更新导致发布失败.md",
  "updated_sop_path": "02_SOPs/Release/SOP - iOS 发布前检查.md"
}
```

### POST /v1/triggers

Create TriggerEvent.

Request:

```json
{
  "id": "trg_01JX9ZX51C",
  "source": "git",
  "created_at": "2026-05-22T15:40:00+09:00",
  "payload": {
    "repo": "my-ios-app",
    "event": "checkout",
    "branch": "release/2.5.0"
  }
}
```

Response:

```json
{
  "ok": true,
  "detected_scene": "iOS 发布",
  "confidence": 0.91,
  "matched_sops": [
    {
      "id": "sop_ios_release",
      "title": "SOP - iOS 发布前检查",
      "path": "02_SOPs/Release/SOP - iOS 发布前检查.md"
    }
  ],
  "action": "notified"
}
```

### GET /v1/search?q={query}

Search Pit/SOP/Scene.

Response:

```json
{
  "query": "证书过期",
  "results": [
    {
      "type": "pit",
      "title": "证书过期导致 CI 打包失败",
      "path": "01_Pits/2026/证书过期导致 CI 打包失败.md",
      "score": 0.89
    }
  ]
}
```

---

## 8. Markdown Spec

## 8.1 Pit Frontmatter

```yaml
type: pit
id: pit_01JX9ZQH2A
created: 2026-05-22T15:30:00+09:00
source: mobile_voice
status: processed
scenario: iOS 发布
risk: high
recurrence: high
sop: "[[SOP - iOS 发布前检查]]"
tags:
  - pit
  - ios
  - release
  - ci
```

Required fields:

- `type`
- `id`
- `created`
- `source`
- `status`
- `scenario`
- `risk`

## 8.2 SOP Frontmatter

```yaml
type: sop
id: sop_ios_release
version: 3
status: active
risk: high
scenarios:
  - iOS 发布
  - App Store 提审
triggers:
  - release
  - App Store
  - TestFlight
  - production
related_pits:
  - "[[CI secret 未更新导致 production 请求失败]]"
tags:
  - sop
  - ios
  - release
```

Required fields:

- `type`
- `id`
- `version`
- `status`
- `risk`
- `scenarios`
- `triggers`

## 8.3 Scene Frontmatter

```yaml
type: scene
id: scene_ios_release
name: iOS 发布
risk: high
matched_sops:
  - "[[SOP - iOS 发布前检查]]"
trigger_keywords:
  - 上线
  - 发布
  - release
  - App Store
  - production
sources:
  - git
  - calendar
  - browser
  - manual
tags:
  - scene
  - ios
```

---

## 9. Prompt Contract

## 9.1 Pit Extraction Prompt Output

AI 必须输出严格 JSON。

```json
{
  "classification": "pit",
  "confidence": 0.92,
  "pit": {
    "title": "string",
    "scenario": "string",
    "symptom": "string",
    "root_cause": "string",
    "fix": "string",
    "prevention_rule": "string",
    "risk_level": "low|medium|high",
    "recurrence_probability": "low|medium|high",
    "tags": ["string"],
    "trigger_keywords": ["string"]
  },
  "sop_action": {
    "type": "update_existing|create_new|needs_review|none",
    "sop_title": "string",
    "checklist_items": ["string"],
    "reason": "string"
  }
}
```

## 9.2 SOP Matching Output

```json
{
  "matched": true,
  "confidence": 0.87,
  "matched_sop_id": "sop_ios_release",
  "matched_scene": "iOS 发布",
  "reason": "输入包含 release、production、CI 等发布信号",
  "new_items": [
    "检查 CI secret 是否为最新值"
  ]
}
```

---

## 10. Matching Spec

## 10.1 SOP 匹配因子

| 因子 | 权重 |
|---|---:|
| trigger keyword match | 0.30 |
| scene name match | 0.20 |
| semantic similarity | 0.25 |
| related pit similarity | 0.15 |
| risk boost | 0.10 |

## 10.2 提醒阈值

| score | action |
|---:|---|
| >= 0.85 | 立即通知 |
| 0.65 - 0.85 | 今日建议 |
| 0.45 - 0.65 | App 内弱提示 |
| < 0.45 | 忽略 |

## 10.3 重复提醒抑制

同一 SOP 在以下条件下不重复提醒：

- 同一 source + scene + repo 在 30 分钟内已提醒。
- 用户已选择“忽略本次”。
- 用户已在本次 TriggerEvent 中打开 SOP。

---

## 11. UX Spec

## 11.1 桌面托盘菜单

```text
Pit2SOP
├── 记录一个坑
├── 我要做一件事
├── 搜索 SOP
├── 今日风险
├── 打开 Obsidian Vault
├── 同步状态
└── 设置
```

## 11.2 桌面输入框

输入框支持模式前缀：

```text
pit: 今天 CI secret 忘记更新
做: 我要上线 2.5.0
找: 证书过期怎么处理
```

也支持自然语言自动判断。

## 11.3 手机首页

```text
[按住说话]
[输入文字]
[发送截图/图片]

最近输入
- CI secret 忘记更新    processed
- 客户交付漏测试账号     queued
- 数据库迁移默认值遗漏   delivered
```

## 11.4 通知

通知标题：

```text
检测到高风险场景：iOS 发布
```

通知正文：

```text
建议执行「SOP - iOS 发布前检查」。历史坑点：CI secret、证书、审核账号。
```

按钮：

```text
打开 SOP | 稍后提醒 | 忽略本次
```

---

## 12. Acceptance Criteria by Phase

## Phase 1：桌面 + Obsidian 闭环

必须完成：

- [ ] 设置 Obsidian Vault 路径
- [ ] 初始化目录结构
- [ ] 桌面输入文本 pit
- [ ] AI 结构化 Pit
- [ ] 生成 Pit Markdown
- [ ] 生成或更新 SOP Markdown
- [ ] SQLite 记录状态
- [ ] 可以搜索已生成内容

## Phase 2：手机输入

必须完成：

- [ ] 手机录音
- [ ] 手机文本输入
- [ ] 手机本地队列
- [ ] 手机扫码配对桌面端
- [ ] LAN 发送 CaptureEvent
- [ ] 桌面接收并处理
- [ ] 手机显示 processed 状态

## Phase 3：提醒闭环

必须完成：

- [ ] 手动 doing 触发 SOP 推荐
- [ ] Git hook 触发 release 场景
- [ ] 文件监听触发 migration 场景
- [ ] 桌面通知
- [ ] 点击通知打开 Obsidian SOP
- [ ] 忽略/稍后提醒状态记录

## Phase 4：扩展输入源

建议完成：

- [ ] 浏览器扩展
- [ ] 云中转
- [ ] 日历/提醒事项读取
- [ ] 周报复盘

---

## 13. Open Questions

当前未定问题：

1. 第一版是否只做 Mac，还是 Mac/Windows 同时做？
2. 手机端是否先只做 iOS？
3. AI 服务使用云端 API，还是保留本地模型接口？
4. Vector Index 使用 SQLite 扩展、LanceDB，还是独立向量库？
5. Obsidian 是否需要插件，还是先用纯文件写入？

建议默认答案：

```text
先做 Mac/Windows 桌面 Agent + iOS 手机输入。
AI 先用云端。
Obsidian 先用纯 Markdown 文件写入。
向量索引先用可替换本地缓存。
插件后置。
```

---

## 14. Implementation Checklist

### Desktop Agent

- [ ] App shell
- [ ] Settings
- [ ] Vault selector
- [ ] Local API server
- [ ] SQLite migrations
- [ ] Markdown writer
- [ ] AI processor
- [ ] Notification engine
- [ ] CLI bridge
- [ ] Git hook installer
- [ ] File watcher

### Mobile App

- [ ] Capture UI
- [ ] Audio recorder
- [ ] Text input
- [ ] Local queue
- [ ] QR pairing
- [ ] LAN sender
- [ ] Attachment uploader
- [ ] Status polling

### Knowledge Layer

- [ ] Pit template
- [ ] SOP template
- [ ] Scene template
- [ ] Markdown parser
- [ ] YAML frontmatter parser
- [ ] Link resolver
- [ ] Index builder

### AI Layer

- [ ] Transcription adapter
- [ ] Classification prompt
- [ ] Pit extraction prompt
- [ ] SOP matching prompt
- [ ] SOP patch generator
- [ ] JSON validation
- [ ] Retry and fallback

---

## 15. Final Design Decision

Pit2SOP 应采用以下核心设计：

```text
Phone Capture App
+ Desktop SOP Agent
+ Obsidian Markdown Vault
+ SQLite / Vector Cache
+ Optional Cloud Relay
```

关键原则：

1. 手机只负责低摩擦输入。
2. 桌面端是主脑。
3. Obsidian 是长期知识库。
4. SQLite 和向量索引只是缓存。
5. 每个坑都必须尝试转化为 SOP。
6. 每个 SOP 都要能被场景触发。
7. 自动更新不能破坏人工编辑。

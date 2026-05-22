import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import {
  CheckCircle2,
  FileCheck2,
  FolderOpen,
  KeyRound,
  ListChecks,
  PlugZap,
  RotateCw,
  Save,
  Search,
  Send,
  Settings,
  ShieldAlert,
  Trash2,
  XCircle,
} from "lucide-react";
import "./App.css";

type RiskLevel = "low" | "medium" | "high";

type ProcessingSummary = {
  capture_id: string;
  status: string;
  pit_path?: string | null;
  sop_path?: string | null;
  pending_patch_path?: string | null;
  review_path?: string | null;
  message: string;
};

type DoingMatch = {
  score: number;
  title: string;
  path: string;
  risk: RiskLevel;
  checklist_items: string[];
  reasons: string[];
};

type SearchResult = {
  doc_type: string;
  title: string;
  path: string;
  snippet: string;
};

type PendingPatch = {
  path: string;
  target: string;
  source_pit: string;
  status: string;
  title: string;
};

type PatchActionSummary = {
  path: string;
  target_path?: string | null;
  source_pit?: string | null;
  status: string;
  message: string;
};

type AppStatus = {
  vault_path: string;
  db_path: string;
  ai_provider: string;
  ai_model: string;
  secrets_configured: boolean;
  indexed_docs: number;
  pit_files: number;
  sop_files: number;
};

type DesktopSettings = {
  vault_path?: string | null;
  language: string;
  ai_provider: string;
  ai_model: string;
  ai_base_url?: string | null;
  has_deepseek_api_key: boolean;
};

type SaveSettingsInput = {
  vault_path: string;
  language: string;
  ai_provider: string;
  ai_model: string;
  ai_base_url?: string | null;
};

type SecretSaveSummary = {
  provider: string;
  configured: boolean;
};

type AiHealthCheck = {
  provider: string;
  model: string;
  ok: boolean;
  message: string;
};

type TabKey = "pit" | "doing" | "search" | "pending" | "settings";

const tabs: Array<{ key: TabKey; label: string; icon: typeof ShieldAlert }> = [
  { key: "pit", label: "记录一个坑", icon: ShieldAlert },
  { key: "doing", label: "我要做一件事", icon: ListChecks },
  { key: "search", label: "搜索", icon: Search },
  { key: "pending", label: "Pending", icon: FileCheck2 },
  { key: "settings", label: "设置", icon: Settings },
];

const defaultSettingsForm: SaveSettingsInput = {
  vault_path: "",
  language: "zh-CN",
  ai_provider: "deepseek",
  ai_model: "deepseek-v4-pro",
  ai_base_url: "https://api.deepseek.com",
};

function App() {
  const [activeTab, setActiveTab] = useState<TabKey>("pit");
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [statusError, setStatusError] = useState("");
  const [pitText, setPitText] = useState("");
  const [pitSummary, setPitSummary] = useState<ProcessingSummary | null>(null);
  const [doingText, setDoingText] = useState("");
  const [matches, setMatches] = useState<DoingMatch[]>([]);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [pending, setPending] = useState<PendingPatch[]>([]);
  const [settings, setSettings] = useState<DesktopSettings | null>(null);
  const [settingsForm, setSettingsForm] =
    useState<SaveSettingsInput>(defaultSettingsForm);
  const [apiKey, setApiKey] = useState("");
  const [aiHealth, setAiHealth] = useState<AiHealthCheck | null>(null);
  const [busy, setBusy] = useState("");
  const [notice, setNotice] = useState("");
  const [error, setError] = useState("");

  const selectedTab = useMemo(
    () => tabs.find((tab) => tab.key === activeTab) ?? tabs[0],
    [activeTab],
  );

  useEffect(() => {
    void refreshStatus();
    void refreshSettings();
    void refreshPending();
  }, []);

  async function run<T>(label: string, task: () => Promise<T>) {
    setBusy(label);
    setError("");
    setNotice("");
    try {
      return await task();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      return null;
    } finally {
      setBusy("");
    }
  }

  async function refreshStatus() {
    const next = await invoke<AppStatus>("app_status").catch((cause) => {
      setStatusError(cause instanceof Error ? cause.message : String(cause));
      return null;
    });
    if (next) {
      setStatus(next);
      setStatusError("");
    }
  }

  async function refreshSettings() {
    const next = await invoke<DesktopSettings>("get_settings").catch((cause) => {
      setStatusError(cause instanceof Error ? cause.message : String(cause));
      return null;
    });
    if (next) applySettings(next);
  }

  async function refreshPending() {
    const next = await invoke<PendingPatch[]>("pending_patches").catch(() => []);
    setPending(next);
  }

  async function refreshAll() {
    await refreshStatus();
    await refreshSettings();
    await refreshPending();
  }

  function applySettings(next: DesktopSettings) {
    setSettings(next);
    setSettingsForm({
      vault_path: next.vault_path ?? "",
      language: next.language || "zh-CN",
      ai_provider: next.ai_provider || "deepseek",
      ai_model: next.ai_model || "deepseek-v4-pro",
      ai_base_url: next.ai_base_url ?? "",
    });
    setStatusError("");
  }

  async function submitPit() {
    if (!pitText.trim()) return;
    const summary = await run("pit", () =>
      invoke<ProcessingSummary>("process_pit", { text: pitText }),
    );
    if (summary) {
      setPitSummary(summary);
      setNotice(summary.message);
      setPitText("");
      await refreshPending();
      await refreshStatus();
    }
  }

  async function submitDoing() {
    if (!doingText.trim()) return;
    const next = await run("doing", () =>
      invoke<DoingMatch[]>("doing", { text: doingText }),
    );
    if (next) setMatches(next);
  }

  async function submitSearch() {
    if (!query.trim()) return;
    const next = await run("search", () =>
      invoke<SearchResult[]>("search", { query }),
    );
    if (next) setResults(next);
  }

  async function applyPatch(path: string) {
    const summary = await run("apply", () =>
      invoke<PatchActionSummary>("apply_pending_patch", { path }),
    );
    if (summary) {
      setNotice(summary.message);
      await refreshPending();
      await refreshStatus();
    }
  }

  async function rejectPatch(path: string) {
    const summary = await run("reject", () =>
      invoke<PatchActionSummary>("reject_pending_patch", { path }),
    );
    if (summary) {
      setNotice(summary.message);
      await refreshPending();
      await refreshStatus();
    }
  }

  async function openVault() {
    await run("openVault", () => invoke<void>("open_vault"));
  }

  async function chooseVault() {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: "选择 Pit2SOP Vault",
    });
    if (typeof selected === "string") {
      setSettingsForm((current) => ({ ...current, vault_path: selected }));
    }
  }

  async function saveSettings() {
    const next = await run("saveSettings", () =>
      invoke<AppStatus>("save_settings", { input: settingsForm }),
    );
    if (next) {
      setStatus(next);
      setNotice("设置已保存");
      await refreshSettings();
      await refreshPending();
    }
  }

  async function saveSecret() {
    if (!apiKey.trim()) return;
    const summary = await run("saveSecret", () =>
      invoke<SecretSaveSummary>("save_ai_secret", {
        input: {
          provider: settingsForm.ai_provider,
          api_key: apiKey,
        },
      }),
    );
    if (summary) {
      setApiKey("");
      setNotice(`${summary.provider} secret 已保存`);
      await refreshSettings();
      await refreshStatus();
    }
  }

  async function clearSecret() {
    const summary = await run("clearSecret", () =>
      invoke<SecretSaveSummary>("clear_ai_secret", { provider: "deepseek" }),
    );
    if (summary) {
      setNotice(`${summary.provider} secret 已清除`);
      await refreshSettings();
      await refreshStatus();
    }
  }

  async function testAiProvider() {
    const health = await run("testAi", () =>
      invoke<AiHealthCheck>("test_ai_provider"),
    );
    if (health) {
      setAiHealth(health);
      setNotice(health.message);
    }
  }

  function updateSettingsField<K extends keyof SaveSettingsInput>(
    key: K,
    value: SaveSettingsInput[K],
  ) {
    setSettingsForm((current) => ({ ...current, [key]: value }));
  }

  function updateProvider(provider: string) {
    setSettingsForm((current) => ({
      ...current,
      ai_provider: provider,
      ai_model:
        provider === "heuristic"
          ? "heuristic"
          : current.ai_model === "heuristic"
            ? "deepseek-v4-pro"
            : current.ai_model,
      ai_base_url:
        provider === "heuristic"
          ? "local"
          : current.ai_base_url === "local" || !current.ai_base_url
            ? "https://api.deepseek.com"
            : current.ai_base_url,
    }));
  }

  return (
    <main className="shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="mark">P2</div>
          <div>
            <strong>Pit2SOP</strong>
            <span>V0.2 Desktop</span>
          </div>
        </div>

        <nav className="tabs">
          {tabs.map((tab) => {
            const Icon = tab.icon;
            return (
              <button
                className={tab.key === activeTab ? "tab active" : "tab"}
                key={tab.key}
                onClick={() => setActiveTab(tab.key)}
                type="button"
              >
                <Icon size={17} />
                {tab.label}
              </button>
            );
          })}
        </nav>

        <div className="vault-chip">
          <span>Vault</span>
          <strong>{status ? compactPath(status.vault_path) : "未加载"}</strong>
        </div>
      </aside>

      <section className="workspace">
        <header className="topbar">
          <div>
            <h1>{selectedTab.label}</h1>
            <p>{subtitle(activeTab)}</p>
          </div>
          <div className="top-actions">
            <button className="icon-button" onClick={() => void refreshAll()} type="button">
              <RotateCw size={17} />
              刷新
            </button>
            <button
              className="icon-button"
              disabled={!status?.vault_path}
              onClick={() => void openVault()}
              type="button"
            >
              <FolderOpen size={17} />
              Vault
            </button>
          </div>
        </header>

        {notice && <div className="notice">{notice}</div>}
        {error && <div className="error">{error}</div>}
        {statusError && <div className="error">{statusError}</div>}

        <div className="content">
          {activeTab === "pit" && (
            <section className="panel">
              <textarea
                className="primary-input"
                onChange={(event) => setPitText(event.currentTarget.value)}
                placeholder="今天发生了什么坑？写现象、原因、修复和下次怎么避免。"
                value={pitText}
              />
              <div className="panel-actions">
                <button
                  className="primary-button"
                  disabled={!pitText.trim() || busy === "pit"}
                  onClick={submitPit}
                  type="button"
                >
                  <Send size={18} />
                  记录
                </button>
              </div>
              {pitSummary && <SummaryBlock summary={pitSummary} />}
            </section>
          )}

          {activeTab === "doing" && (
            <section className="panel">
              <textarea
                className="primary-input compact"
                onChange={(event) => setDoingText(event.currentTarget.value)}
                placeholder="我要做什么？例如：我要装 PBS，或者我要上线 2.5.0。"
                value={doingText}
              />
              <div className="panel-actions">
                <button
                  className="primary-button"
                  disabled={!doingText.trim() || busy === "doing"}
                  onClick={submitDoing}
                  type="button"
                >
                  <ListChecks size={18} />
                  检查
                </button>
              </div>
              <div className="results">
                {matches.map((match) => (
                  <article className="result" key={match.path}>
                    <div className="result-head">
                      <strong>{match.title}</strong>
                      <span className={`risk ${match.risk}`}>{match.risk}</span>
                    </div>
                    <small>{match.path}</small>
                    <ul>
                      {match.checklist_items.map((item) => (
                        <li key={item}>{item}</li>
                      ))}
                    </ul>
                    <p>{match.reasons.join("；")}</p>
                  </article>
                ))}
                {matches.length === 0 && <EmptyState text="还没有匹配结果" />}
              </div>
            </section>
          )}

          {activeTab === "search" && (
            <section className="panel">
              <div className="search-row">
                <input
                  onChange={(event) => setQuery(event.currentTarget.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") void submitSearch();
                  }}
                  placeholder="关键词"
                  value={query}
                />
                <button
                  className="primary-button"
                  disabled={!query.trim() || busy === "search"}
                  onClick={submitSearch}
                  type="button"
                >
                  <Search size={18} />
                  搜索
                </button>
              </div>
              <div className="results">
                {results.map((item) => (
                  <article className="result" key={item.path}>
                    <div className="result-head">
                      <strong>{item.title}</strong>
                      <span>{item.doc_type}</span>
                    </div>
                    <small>{item.path}</small>
                    <p>{item.snippet}</p>
                  </article>
                ))}
                {results.length === 0 && <EmptyState text="还没有搜索结果" />}
              </div>
            </section>
          )}

          {activeTab === "pending" && (
            <section className="panel">
              <div className="section-toolbar">
                <strong>{pending.length} 个待确认 patch</strong>
                <button className="icon-button" onClick={refreshPending} type="button">
                  <RotateCw size={17} />
                  刷新
                </button>
              </div>
              <div className="results">
                {pending.map((patch) => (
                  <article className="result pending-item" key={patch.path}>
                    <div>
                      <div className="result-head">
                        <strong>{patch.title}</strong>
                        <span>{patch.status}</span>
                      </div>
                      <small>{patch.path}</small>
                      <p>target: {patch.target}</p>
                      <p>source: {patch.source_pit}</p>
                    </div>
                    <div className="patch-actions">
                      <button
                        className="primary-button"
                        disabled={busy === "apply"}
                        onClick={() => void applyPatch(patch.path)}
                        type="button"
                      >
                        <CheckCircle2 size={18} />
                        应用
                      </button>
                      <button
                        className="danger-button"
                        disabled={busy === "reject"}
                        onClick={() => void rejectPatch(patch.path)}
                        type="button"
                      >
                        <XCircle size={18} />
                        拒绝
                      </button>
                    </div>
                  </article>
                ))}
                {pending.length === 0 && <EmptyState text="没有待确认 patch" />}
              </div>
            </section>
          )}

          {activeTab === "settings" && (
            <section className="panel settings-panel">
              <div className="settings-section">
                <div className="section-toolbar">
                  <strong>Vault</strong>
                  <button
                    className="icon-button"
                    disabled={busy === "saveSettings"}
                    onClick={() => void saveSettings()}
                    type="button"
                  >
                    <Save size={17} />
                    保存
                  </button>
                </div>
                <label className="field wide">
                  <span>Vault Path</span>
                  <div className="path-row">
                    <input
                      onChange={(event) =>
                        updateSettingsField("vault_path", event.currentTarget.value)
                      }
                      value={settingsForm.vault_path}
                    />
                    <button
                      className="icon-button"
                      onClick={() => void chooseVault()}
                      type="button"
                    >
                      <FolderOpen size={17} />
                      选择
                    </button>
                  </div>
                </label>
                <label className="field">
                  <span>Language</span>
                  <input
                    onChange={(event) =>
                      updateSettingsField("language", event.currentTarget.value)
                    }
                    value={settingsForm.language}
                  />
                </label>
              </div>

              <div className="settings-section">
                <div className="section-toolbar">
                  <strong>AI</strong>
                  <button
                    className="icon-button"
                    disabled={busy === "testAi"}
                    onClick={() => void testAiProvider()}
                    type="button"
                  >
                    <PlugZap size={17} />
                    测试
                  </button>
                </div>
                <div className="form-grid">
                  <label className="field">
                    <span>Provider</span>
                    <select
                      onChange={(event) => updateProvider(event.currentTarget.value)}
                      value={settingsForm.ai_provider}
                    >
                      <option value="deepseek">deepseek</option>
                      <option value="heuristic">heuristic</option>
                    </select>
                  </label>
                  <label className="field">
                    <span>Model</span>
                    <input
                      onChange={(event) =>
                        updateSettingsField("ai_model", event.currentTarget.value)
                      }
                      value={settingsForm.ai_model}
                    />
                  </label>
                  <label className="field wide">
                    <span>Base URL</span>
                    <input
                      onChange={(event) =>
                        updateSettingsField("ai_base_url", event.currentTarget.value)
                      }
                      value={settingsForm.ai_base_url ?? ""}
                    />
                  </label>
                </div>

                <label className="field wide">
                  <span>API Key</span>
                  <div className="path-row">
                    <input
                      onChange={(event) => setApiKey(event.currentTarget.value)}
                      placeholder={
                        settings?.has_deepseek_api_key ? "configured" : "missing"
                      }
                      type="password"
                      value={apiKey}
                    />
                    <button
                      className="primary-button"
                      disabled={!apiKey.trim() || busy === "saveSecret"}
                      onClick={() => void saveSecret()}
                      type="button"
                    >
                      <KeyRound size={17} />
                      保存
                    </button>
                    <button
                      className="danger-button"
                      disabled={busy === "clearSecret"}
                      onClick={() => void clearSecret()}
                      type="button"
                    >
                      <Trash2 size={17} />
                      清除
                    </button>
                  </div>
                </label>

                {aiHealth && (
                  <div className={aiHealth.ok ? "health ok" : "health bad"}>
                    <strong>{aiHealth.ok ? "ok" : "failed"}</strong>
                    <span>
                      {aiHealth.provider} / {aiHealth.model}
                    </span>
                    <p>{aiHealth.message}</p>
                  </div>
                )}
              </div>

              <div className="settings-grid">
                <Metric label="Vault" value={status?.vault_path ?? "-"} />
                <Metric label="DB" value={status?.db_path ?? "-"} />
                <Metric
                  label="AI"
                  value={`${status?.ai_provider ?? "-"} / ${status?.ai_model ?? "-"}`}
                />
                <Metric
                  label="Secrets"
                  value={status?.secrets_configured ? "configured" : "missing"}
                />
                <Metric label="Indexed docs" value={String(status?.indexed_docs ?? 0)} />
                <Metric label="Pits" value={String(status?.pit_files ?? 0)} />
                <Metric label="SOPs" value={String(status?.sop_files ?? 0)} />
              </div>
            </section>
          )}
        </div>
      </section>
    </main>
  );
}

function SummaryBlock({ summary }: { summary: ProcessingSummary }) {
  return (
    <div className="summary">
      <div className="result-head">
        <strong>{summary.status}</strong>
        <span>{summary.capture_id}</span>
      </div>
      {summary.pit_path && <p>Pit: {summary.pit_path}</p>}
      {summary.sop_path && <p>SOP: {summary.sop_path}</p>}
      {summary.pending_patch_path && <p>Pending: {summary.pending_patch_path}</p>}
      {summary.review_path && <p>Review: {summary.review_path}</p>}
    </div>
  );
}

function EmptyState({ text }: { text: string }) {
  return <div className="empty">{text}</div>;
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function compactPath(path: string) {
  const parts = path.split("/");
  if (parts.length <= 3) return path;
  return `${parts[parts.length - 3]}/${parts[parts.length - 2]}/${parts[parts.length - 1]}`;
}

function subtitle(tab: TabKey) {
  switch (tab) {
    case "pit":
      return "把一次失误转成 Pit，并让 core 决定 SOP 或 pending patch。";
    case "doing":
      return "输入即将做的事，检查历史 SOP 是否需要提醒。";
    case "search":
      return "从 SQLite 缓存搜索 Pit、SOP 和场景文档。";
    case "pending":
      return "处理 AI 不确定的 SOP patch，应用或拒绝都会关闭源 Pit review。";
    case "settings":
      return "查看当前本地配置和索引状态。";
  }
}

export default App;

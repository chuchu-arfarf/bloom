import { StrictMode, useCallback, useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { ArrowUp, Folder, Plus, Square, X } from "lucide-react";
import { initI18n, t, useTranslation } from "./i18n";
import { initTheme } from "./theme";
import type { ChatItem, ClaudeTab, PanelState, SessionView, UsageInfo } from "./claude/types";
import { fiveHour, folderName } from "./claude/types";
import { describeCall, errorText, resetTime, usageLabel } from "./claude/words";
import "./Claude.css";

initI18n();
const appWindow = getCurrentWebviewWindow();

const EMPTY_SESSION: SessionView = { items: [], status: "idle", approval: null, folder: null };
const EMPTY_STATE: PanelState = { chat: EMPTY_SESSION, code: EMPTY_SESSION, installed: true };

function UsageRing({ percent }: { percent: number }) {
	const r = 8;
	const c = 2 * Math.PI * r;
	const p = Math.max(0, Math.min(100, percent));
	return (
		<svg width="20" height="20" viewBox="0 0 20 20" aria-hidden="true">
			<circle cx="10" cy="10" r={r} className="claude-ring-track" />
			<circle
				cx="10"
				cy="10"
				r={r}
				className={`claude-ring-fill ${p >= 85 ? "high" : ""}`}
				strokeDasharray={`${(p / 100) * c} ${c}`}
				transform="rotate(-90 10 10)"
			/>
		</svg>
	);
}

function Item({ item }: { item: ChatItem }) {
	switch (item.kind) {
		case "user":
			return <div className="claude-user">{item.text}</div>;
		case "assistant":
			return <div className="claude-assistant">{item.text}</div>;
		case "tool":
			return (
				<div className="claude-tool" title={item.call.detail}>
					▸ {describeCall(item.call)}
				</div>
			);
		case "decision":
			return (
				<div className="claude-note">
					{item.allowed
						? t("claude.decision.allowed", { action: describeCall(item.call) })
						: t("claude.decision.denied", { action: describeCall(item.call) })}
				</div>
			);
		case "error":
			return (
				<div className="claude-error">
					{item.code ? errorText(item.code) : null}
					{item.text ? <div className="claude-error-detail">{item.text}</div> : null}
				</div>
			);
	}
}

function ClaudePanel() {
	useTranslation();
	const [state, setState] = useState<PanelState>(EMPTY_STATE);
	const [tab, setTab] = useState<ClaudeTab>("chat");
	const [draft, setDraft] = useState("");
	const [usage, setUsage] = useState<UsageInfo | null>(null);
	const pickingFolder = useRef(false);
	const scrollRef = useRef<HTMLDivElement>(null);
	const inputRef = useRef<HTMLTextAreaElement>(null);

	useEffect(() => initTheme(), []);

	useEffect(() => {
		invoke<PanelState>("claude_state").then(setState).catch(console.error);
		invoke<UsageInfo | null>("claude_usage", { refresh: false }).then(setUsage).catch(console.error);
		const unlisteners = [
			listen<PanelState>("claude-state", (e) => setState(e.payload)),
			listen<UsageInfo>("claude-usage", (e) => setUsage(e.payload)),
			listen<string>("claude-open-tab", (e) => setTab(e.payload === "code" ? "code" : "chat"))
		];
		return () => {
			unlisteners.forEach((u) => u.then((f) => f()));
		};
	}, []);

	// Close when you click elsewhere, like other Bloom panels; not while a folder picker is open.
	useEffect(() => {
		const unlisten = appWindow.onFocusChanged(({ payload: focused }) => {
			if (focused) {
				inputRef.current?.focus();
			} else if (!pickingFolder.current) {
				invoke("hide_claude_window").catch(console.error);
			}
		});
		return () => {
			unlisten.then((f) => f());
		};
	}, []);

	useEffect(() => {
		const onKey = (e: KeyboardEvent) => {
			if (e.key === "Escape") invoke("hide_claude_window").catch(console.error);
		};
		window.addEventListener("keydown", onKey);
		return () => window.removeEventListener("keydown", onKey);
	}, []);

	const session = state[tab];

	useEffect(() => {
		scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight });
	}, [session.items.length, session.approval, tab]);

	const chooseFolder = useCallback(async () => {
		pickingFolder.current = true;
		try {
			const { open } = await import("@tauri-apps/plugin-dialog");
			const selected = await open({
				title: t("claude.code.chooseFolderTitle"),
				directory: true,
				multiple: false
			});
			if (typeof selected === "string") {
				await invoke("claude_set_folder", { folder: selected });
				await invoke("save_setting", { key: "bloom-claude-folder", value: selected });
			}
		} catch (e) {
			console.error("Folder picker failed:", e);
		} finally {
			pickingFolder.current = false;
			appWindow.setFocus().catch(() => {});
		}
	}, []);

	const send = useCallback(async () => {
		const text = draft.trim();
		if (!text) return;
		if (tab === "code" && !state.code.folder) {
			await chooseFolder();
			return;
		}
		setDraft("");
		invoke("claude_send", { tab, text }).catch(console.error);
	}, [draft, tab, state.code.folder, chooseFolder]);

	const answer = (allow: boolean, always = false) =>
		invoke("claude_answer", { tab, allow, always }).catch(console.error);

	const five = fiveHour(usage);
	const busy = session.status === "working";
	const waiting = session.status === "needs_approval";

	return (
		<div className="claude-panel">
			<header className="claude-header">
				<div className="claude-tabs" role="tablist">
					{(["chat", "code"] as const).map((id) => (
						<button
							key={id}
							role="tab"
							aria-selected={tab === id}
							className={tab === id ? "active" : ""}
							onClick={() => setTab(id)}
						>
							{id === "chat" ? t("claude.tabs.chat") : t("claude.tabs.code")}
						</button>
					))}
				</div>
				{five && (
					<div
						className="claude-usage"
						title={usage?.windows
							.map((w) => t("claude.usage.line", { label: usageLabel(w), percent: Math.round(w.percent) }))
							.join("\n")}
					>
						<UsageRing percent={five.percent} />
						<span>{t("claude.usage.percent", { percent: Math.round(five.percent) })}</span>
						{five.resets_at && resetTime(five.resets_at) && (
							<span className="claude-muted">
								{t("claude.usage.resets", { time: resetTime(five.resets_at) ?? "" })}
							</span>
						)}
					</div>
				)}
				<button
					className="claude-icon-button"
					aria-label={t("claude.actions.newConversation")}
					title={t("claude.actions.newConversation")}
					onClick={() => invoke("claude_new", { tab }).catch(console.error)}
				>
					<Plus size={16} />
				</button>
				<button
					className="claude-icon-button"
					aria-label={t("claude.actions.close")}
					title={t("claude.actions.close")}
					onClick={() => invoke("hide_claude_window").catch(console.error)}
				>
					<X size={16} />
				</button>
			</header>

			{tab === "code" && (
				<button className="claude-folder" onClick={chooseFolder} title={state.code.folder ?? undefined}>
					<Folder size={14} />
					<span className="claude-folder-name">
						{state.code.folder ? folderName(state.code.folder) : t("claude.code.chooseFolder")}
					</span>
					<span className="claude-muted">{t("claude.code.change")}</span>
				</button>
			)}

			<div className="claude-messages" ref={scrollRef}>
				{!state.installed && <div className="claude-error">{errorText("not_installed")}</div>}
				{session.items.length === 0 && state.installed && (
					<div className="claude-empty">
						{tab === "chat"
							? t("claude.chat.empty")
							: state.code.folder
								? t("claude.code.emptyWithFolder", { folder: folderName(state.code.folder) })
								: t("claude.code.emptyNoFolder")}
					</div>
				)}
				{session.items.map((item, index) => (
					<Item key={index} item={item} />
				))}
			</div>

			{session.approval && (
				<div className="claude-approval" role="alertdialog" aria-label={t("claude.approval.title")}>
					<div className="claude-approval-title">{t("claude.approval.title")}</div>
					<div className="claude-approval-summary">{describeCall(session.approval.call)}</div>
					{session.approval.call.detail && session.approval.call.detail !== session.approval.call.target && (
						<pre className="claude-approval-detail">{session.approval.call.detail}</pre>
					)}
					<div className="claude-approval-buttons">
						<button className="primary" onClick={() => answer(true)} autoFocus>
							{t("claude.approval.allow")}
						</button>
						<button onClick={() => answer(true, true)}>{t("claude.approval.allowAlways")}</button>
						<button className="danger" onClick={() => answer(false)}>
							{t("claude.approval.deny")}
						</button>
					</div>
				</div>
			)}

			{busy && (
				<div className="claude-working">
					<span className="claude-dots" aria-hidden="true" />
					<span>{tab === "chat" ? t("claude.status.thinking") : t("claude.status.working")}</span>
					<button onClick={() => invoke("claude_interrupt", { tab }).catch(console.error)}>
						<Square size={11} /> {t("claude.actions.stop")}
					</button>
				</div>
			)}

			<div className="claude-input">
				<textarea
					ref={inputRef}
					value={draft}
					rows={1}
					disabled={waiting}
					placeholder={tab === "chat" ? t("claude.chat.placeholder") : t("claude.code.placeholder")}
					aria-label={t("claude.input.label")}
					onChange={(e) => setDraft(e.target.value)}
					onKeyDown={(e) => {
						if (e.key === "Enter" && !e.shiftKey) {
							e.preventDefault();
							send();
						}
					}}
				/>
				<button
					className="claude-send"
					aria-label={t("claude.actions.send")}
					disabled={!draft.trim() || waiting}
					onClick={send}
				>
					<ArrowUp size={16} />
				</button>
			</div>
			<div className="claude-footer">
				{tab === "chat" ? t("claude.chat.footer") : t("claude.code.footer")}
			</div>
		</div>
	);
}

createRoot(document.getElementById("root") as HTMLElement).render(
	<StrictMode>
		<ClaudePanel />
	</StrictMode>
);

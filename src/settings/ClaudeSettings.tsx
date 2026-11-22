import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Bot, Cpu, FolderSearch, Gauge, Keyboard, MessageSquare, RotateCcw } from "lucide-react";
import { SettingRow } from "./SettingRow";
import { t, useTranslation } from "../i18n";

const MODELS = ["", "sonnet", "opus", "haiku"] as const;

function readBool(key: string): boolean {
	return localStorage.getItem(key) !== "false";
}

async function save(key: string, value: string | null) {
	try {
		await invoke("save_setting", { key, value });
		if (value === null) localStorage.removeItem(key);
		else localStorage.setItem(key, value);
	} catch (error) {
		console.error(`Failed to save ${key}:`, error);
	}
}

/** Settings for the Claude panel (src/Claude.tsx) and the Island usage widget. */
export function ClaudeSettings() {
	useTranslation();
	const [cliPath, setCliPath] = useState<string | null>(null);
	const [model, setModel] = useState(localStorage.getItem("bloom-claude-model") ?? "");
	const [usageEnabled, setUsageEnabled] = useState(readBool("bloom-claude-usage-enabled"));
	const [hotkeyEnabled, setHotkeyEnabled] = useState(readBool("bloom-claude-hotkey-enabled"));

	const refreshPath = () =>
		invoke<string | null>("claude_cli_path")
			.then(setCliPath)
			.catch(() => setCliPath(null));

	useEffect(() => {
		refreshPath();
	}, []);

	const chooseExe = async () => {
		try {
			const { open } = await import("@tauri-apps/plugin-dialog");
			const selected = await open({
				title: t("claude.settings.chooseExeTitle"),
				multiple: false,
				directory: false,
				filters: [{ name: "claude.exe", extensions: ["exe"] }]
			});
			if (typeof selected === "string") {
				await save("bloom-claude-path", selected);
				refreshPath();
			}
		} catch (error) {
			console.error("File picker failed:", error);
		}
	};

	return (
		<div className="setting-group">
			<SettingRow
				icon={MessageSquare}
				label={t("claude.settings.openPanel")}
				desc={t("claude.settings.openPanelDesc")}
				action
				onClick={() => invoke("open_claude_window", { tab: null }).catch(console.error)}
			/>
			<SettingRow
				icon={Bot}
				label={t("claude.settings.location")}
				desc={cliPath ?? t("claude.settings.notFound")}
				action
				onClick={chooseExe}
			/>
			<SettingRow
				icon={RotateCcw}
				label={t("claude.settings.findAutomatically")}
				desc={t("claude.settings.findAutomaticallyDesc")}
				action
				onClick={async () => {
					await save("bloom-claude-path", null);
					refreshPath();
				}}
			/>
			<SettingRow icon={Cpu} label={t("claude.settings.model")} desc={t("claude.settings.modelDesc")}>
				<select
					className="settings-select"
					value={model}
					aria-label={t("claude.settings.model")}
					onChange={(e) => {
						setModel(e.target.value);
						save("bloom-claude-model", e.target.value || null);
					}}
				>
					{MODELS.map((id) => (
						<option key={id || "default"} value={id}>
							{id === "" ? t("claude.settings.modelDefault") : id.charAt(0).toUpperCase() + id.slice(1)}
						</option>
					))}
				</select>
			</SettingRow>
			<SettingRow icon={Gauge} label={t("claude.settings.usage")} desc={t("claude.settings.usageDesc")}>
				<label className="toggle-switch">
					<input
						type="checkbox"
						checked={usageEnabled}
						onChange={() => {
							const next = !usageEnabled;
							setUsageEnabled(next);
							save("bloom-claude-usage-enabled", String(next));
						}}
					/>
					<span className="slider"></span>
				</label>
			</SettingRow>
			<SettingRow
				icon={Keyboard}
				label={t("claude.settings.hotkey")}
				desc={t("claude.settings.hotkeyDesc")}
			>
				<label className="toggle-switch">
					<input
						type="checkbox"
						checked={hotkeyEnabled}
						onChange={() => {
							const next = !hotkeyEnabled;
							setHotkeyEnabled(next);
							save("bloom-claude-hotkey-enabled", String(next));
						}}
					/>
					<span className="slider"></span>
				</label>
			</SettingRow>
			<SettingRow
				icon={FolderSearch}
				label={t("claude.settings.folder")}
				desc={localStorage.getItem("bloom-claude-folder") ?? t("claude.settings.folderNone")}
				divider={false}
			/>
		</div>
	);
}

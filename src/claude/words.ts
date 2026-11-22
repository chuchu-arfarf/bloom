import { t } from "../i18n";
import type { ToolCall, UsageWindow } from "./types";

/** "Edit main.rs", "Run npm test"… worded per tool, with literal keys for i18n:check. */
export function describeCall(call: ToolCall): string {
	const target = call.target;
	switch (call.tool) {
		case "Bash":
		case "PowerShell":
			return t("claude.tool.run", { target });
		case "Edit":
		case "MultiEdit":
		case "NotebookEdit":
			return t("claude.tool.edit", { target });
		case "Write":
			return t("claude.tool.write", { target });
		case "Read":
			return t("claude.tool.read", { target });
		case "Glob":
			return t("claude.tool.findFiles", { target });
		case "Grep":
			return t("claude.tool.search", { target });
		case "WebFetch":
			return t("claude.tool.openPage", { target });
		case "WebSearch":
			return t("claude.tool.webSearch", { target });
		case "Task":
		case "Agent":
			return t("claude.tool.helper", { target });
		default:
			return target ? t("claude.tool.other", { tool: call.tool, target }) : call.tool;
	}
}

export function errorText(code: string | null): string {
	switch (code) {
		case "not_installed":
			return t("claude.errors.notInstalled");
		case "no_folder":
			return t("claude.errors.noFolder");
		case "max_turns":
			return t("claude.errors.maxTurns");
		case "stopped":
			return t("claude.errors.stopped");
		default:
			return t("claude.errors.failed");
	}
}

export function usageLabel(window: UsageWindow): string {
	switch (window.id) {
		case "five_hour":
			return t("claude.usage.fiveHour");
		case "seven_day":
			return t("claude.usage.weekly");
		case "seven_day_opus":
			return t("claude.usage.weeklyModel", { model: "Opus" });
		case "seven_day_sonnet":
			return t("claude.usage.weeklyModel", { model: "Sonnet" });
		default:
			return t("claude.usage.weeklyModel", { model: window.model ?? "" });
	}
}

export function resetTime(iso: string | null): string | null {
	if (!iso) return null;
	const date = new Date(iso);
	if (Number.isNaN(date.getTime())) return null;
	const sameDay = date.toDateString() === new Date().toDateString();
	return sameDay
		? date.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })
		: date.toLocaleString([], { weekday: "short", hour: "numeric", minute: "2-digit" });
}

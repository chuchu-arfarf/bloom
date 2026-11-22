// Shapes sent by src-tauri/src/claude.rs and claude_protocol.rs.

export type ClaudeTab = "chat" | "code";
export type SessionStatus = "idle" | "working" | "needs_approval" | "error";

export interface ToolCall {
	tool: string;
	target: string;
	detail: string;
}

export type ChatItem =
	| { kind: "user"; text: string }
	| { kind: "assistant"; text: string }
	| { kind: "tool"; call: ToolCall }
	| { kind: "decision"; allowed: boolean; call: ToolCall }
	| { kind: "error"; text: string | null; code: string | null };

export interface Approval {
	request_id: string;
	call: ToolCall;
}

export interface SessionView {
	items: ChatItem[];
	status: SessionStatus;
	approval: Approval | null;
	folder: string | null;
}

export interface PanelState {
	chat: SessionView;
	code: SessionView;
	installed: boolean;
}

export interface UsageWindow {
	id: "five_hour" | "seven_day" | "seven_day_opus" | "seven_day_sonnet" | "model";
	model: string | null;
	percent: number;
	resets_at: string | null;
}

export interface UsageInfo {
	plan: string | null;
	available: boolean;
	windows: UsageWindow[];
	error: "not_installed" | "no_answer" | "no_plan" | "failed" | null;
	checked_at: number;
}

export function fiveHour(usage: UsageInfo | null): UsageWindow | null {
	return usage?.windows.find((w) => w.id === "five_hour") ?? null;
}

export function folderName(folder: string | null): string {
	if (!folder) return "";
	const parts = folder.split(/[\\/]/).filter(Boolean);
	return parts[parts.length - 1] ?? folder;
}

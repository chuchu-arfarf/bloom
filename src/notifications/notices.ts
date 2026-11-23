import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** A Windows notification, as the backend's `notifications` module sends it. */
export interface Notice {
	id: number;
	app_id: string;
	/** The sending app's name. */
	app: string;
	title: string;
	body: string;
	/** Arrival time, Unix milliseconds. */
	at: number;
}

export interface NoticeList {
	items: Notice[];
	/** Why Windows' notifications can't be read, when they can't. */
	error: string | null;
}

export interface NoticeApp {
	id: string;
	name: string;
}

export const MUTED_KEY = "bloom-notifications-muted";
export const POPUPS_KEY = "bloom-notifications-popups";

/** Apps whose notifications don't pop up (they still reach the drawer). */
export function mutedApps(): Set<string> {
	try {
		const parsed: unknown = JSON.parse(localStorage.getItem(MUTED_KEY) ?? "[]");
		return new Set(Array.isArray(parsed) ? parsed.filter((id) => typeof id === "string") : []);
	} catch {
		return new Set();
	}
}

/** Whether a new notification from this app should pop up on the Island. */
export function shouldPopUp(notice: Notice): boolean {
	return localStorage.getItem(POPUPS_KEY) !== "false" && !mutedApps().has(notice.app_id);
}

/** The notification center's list, kept current. Empty while `enabled` is off. */
export function useNotices(enabled: boolean): NoticeList {
	const [list, setList] = useState<NoticeList>({ items: [], error: null });
	useEffect(() => {
		if (!enabled) {
			setList({ items: [], error: null });
			return;
		}
		let alive = true;
		invoke<NoticeList>("notifications_list")
			.then((next) => alive && setList(next))
			.catch(console.error);
		const unlisten = listen<NoticeList>("notifications-changed", (e) => setList(e.payload));
		return () => {
			alive = false;
			unlisten.then((fn) => fn());
		};
	}, [enabled]);
	return list;
}

const icons = new Map<string, Promise<string | null>>();

/** The sending app's icon (a data URL), or null when Windows has none. */
export function useAppIcon(appId: string): string | null {
	const [icon, setIcon] = useState<string | null>(null);
	useEffect(() => {
		let alive = true;
		let request = icons.get(appId);
		if (!request) {
			request = invoke<string | null>("notification_app_icon", { appId }).catch(() => null);
			icons.set(appId, request);
		}
		request.then((data) => alive && setIcon(data));
		return () => {
			alive = false;
		};
	}, [appId]);
	return icon;
}

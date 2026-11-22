import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useSettingsSync } from "./useSettingsSync";
import { eventsBetween, type CalendarEvent, type CalendarFeed } from "../calendar/events";

const REFRESH_MS = 15 * 60 * 1000;
const DAY_MS = 86_400_000;

export type CalendarStatus = "none" | "loading" | "ok" | "error";

/**
 * Events from the iCal addresses in `bloom-calendar-urls` (Settings →
 * Integrations → Calendars), downloaded by the backend every 15 minutes.
 * Covers the past month to two months ahead, for the month grid's dots.
 */
export function useCalendarEvents() {
	const [urls, setUrls] = useState(() => localStorage.getItem("bloom-calendar-urls") ?? "");
	const [events, setEvents] = useState<CalendarEvent[]>([]);
	const [status, setStatus] = useState<CalendarStatus>("none");
	const [updatedAt, setUpdatedAt] = useState<number | null>(null);

	useSettingsSync({
		"bloom-calendar-urls": { set: (value: unknown) => setUrls(String(value ?? "")), defaultValue: "" }
	});

	useEffect(() => {
		if (!urls.trim()) {
			setEvents([]);
			setStatus("none");
			return;
		}
		let cancelled = false;
		const load = async () => {
			setStatus((current) => (current === "ok" ? current : "loading"));
			try {
				const feeds = await invoke<CalendarFeed[]>("calendar_fetch");
				if (cancelled) return;
				const today = new Date();
				today.setHours(0, 0, 0, 0);
				const from = new Date(today.getTime() - 31 * DAY_MS);
				const to = new Date(today.getTime() + 62 * DAY_MS);
				setEvents(eventsBetween(feeds, from, to));
				setStatus(feeds.length > 0 && feeds.every((f) => f.error === null) ? "ok" : "error");
				setUpdatedAt(Date.now());
			} catch (error) {
				console.error("Calendar refresh failed:", error);
				if (!cancelled) setStatus("error");
			}
		};
		void load();
		const timer = setInterval(load, REFRESH_MS);
		return () => {
			cancelled = true;
			clearInterval(timer);
		};
	}, [urls]);

	return { events, status, configured: urls.trim().length > 0, updatedAt };
}

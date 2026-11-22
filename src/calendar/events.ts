import ICAL from "ical.js";

/** One occurrence of a calendar event, in local time. Titles come from the user's calendar. */
export interface CalendarEvent {
	id: string;
	title: string;
	location: string;
	start: number;
	end: number;
	allDay: boolean;
}

export interface CalendarFeed {
	text: string | null;
	error: "invalid_url" | "download_failed" | null;
}

const MAX_OCCURRENCES = 2000;

/**
 * Expands every event (recurrences and their exceptions included) that
 * overlaps [from, to). Calendars that fail to parse are skipped.
 */
export function eventsBetween(feeds: CalendarFeed[], from: Date, to: Date): CalendarEvent[] {
	const result: CalendarEvent[] = [];
	const rangeStart = from.getTime();
	const rangeEnd = to.getTime();

	feeds.forEach((feed, feedIndex) => {
		if (!feed.text) return;
		let calendar: InstanceType<typeof ICAL.Component>;
		try {
			calendar = new ICAL.Component(ICAL.parse(feed.text));
		} catch (error) {
			console.warn("Calendar could not be read:", error);
			return;
		}

		for (const zone of calendar.getAllSubcomponents("vtimezone")) {
			try {
				ICAL.TimezoneService.register(zone);
			} catch {
				// A broken time zone block only affects its own events.
			}
		}

		const vevents = calendar.getAllSubcomponents("vevent");
		const exceptions = new Map<string, InstanceType<typeof ICAL.Event>[]>();
		const masters: InstanceType<typeof ICAL.Event>[] = [];
		for (const vevent of vevents) {
			try {
				const event = new ICAL.Event(vevent);
				if (event.isRecurrenceException()) {
					const list = exceptions.get(event.uid) ?? [];
					list.push(event);
					exceptions.set(event.uid, list);
				} else {
					masters.push(event);
				}
			} catch {
				// Skip events this parser can't read.
			}
		}

		for (const event of masters) {
			try {
				if (String(event.component.getFirstPropertyValue("status") ?? "").toUpperCase() === "CANCELLED")
					continue;
				for (const exception of exceptions.get(event.uid) ?? []) event.relateException(exception);

				const push = (start: InstanceType<typeof ICAL.Time>, end: InstanceType<typeof ICAL.Time>, item: InstanceType<typeof ICAL.Event>) => {
					const startMs = start.toJSDate().getTime();
					let endMs = end.toJSDate().getTime();
					if (endMs <= startMs) endMs = start.isDate ? startMs + 86_400_000 : startMs;
					if (endMs <= rangeStart || startMs >= rangeEnd) return;
					result.push({
						id: `${feedIndex}:${item.uid}:${startMs}`,
						title: (item.summary || "").trim(),
						location: (item.location || "").trim(),
						start: startMs,
						end: endMs,
						allDay: start.isDate
					});
				};

				if (!event.isRecurring()) {
					push(event.startDate, event.endDate ?? event.startDate, event);
					continue;
				}

				const iterator = event.iterator();
				let next: InstanceType<typeof ICAL.Time> | null;
				let count = 0;
				while ((next = iterator.next()) && count++ < MAX_OCCURRENCES) {
					if (next.toJSDate().getTime() >= rangeEnd) break;
					const details = event.getOccurrenceDetails(next);
					push(details.startDate, details.endDate, details.item);
				}
			} catch (error) {
				console.warn("Calendar event could not be expanded:", error);
			}
		}
	});

	return result.sort((a, b) => a.start - b.start || a.title.localeCompare(b.title));
}

/** Events on the given local day, all-day ones first. */
export function eventsOn(events: CalendarEvent[], day: Date): CalendarEvent[] {
	const start = new Date(day.getFullYear(), day.getMonth(), day.getDate()).getTime();
	const end = start + 86_400_000;
	return events
		.filter((e) => e.start < end && e.end > start)
		.sort((a, b) => Number(b.allDay) - Number(a.allDay) || a.start - b.start);
}

/** Local day keys (YYYY-M-D) that have at least one event, for the month grid. */
export function busyDays(events: CalendarEvent[]): Set<string> {
	const days = new Set<string>();
	for (const e of events) {
		const d = new Date(e.start);
		const last = new Date(Math.max(e.start, e.end - 1));
		while (d <= last) {
			days.add(`${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`);
			d.setDate(d.getDate() + 1);
		}
	}
	return days;
}

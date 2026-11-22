import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { CalendarDays, ListTodo, Plus, Timer, X } from "lucide-react";
import { getLocale, t, useTranslation } from "../i18n";
import { useSettingsSync } from "../hooks/useSettingsSync";
import { eventsOn, type CalendarEvent } from "../calendar/events";
import type { CalendarStatus } from "../hooks/useCalendarEvents";
import "./CalendarSide.css";

type SideView = "timer" | "today" | "tasks";
const VIEW_KEY = "bloom-calendar-side-view";

interface Task {
	id: string;
	text: string;
	done: boolean;
}

function readTasks(raw: unknown): Task[] {
	try {
		const parsed = typeof raw === "string" ? JSON.parse(raw) : raw;
		if (!Array.isArray(parsed)) return [];
		return parsed
			.filter((task) => task && typeof task.text === "string")
			.map((task, index) => ({
				id: typeof task.id === "string" ? task.id : `t${index}`,
				text: task.text,
				done: task.done === true
			}));
	} catch {
		return [];
	}
}

function formatTime(ms: number): string {
	return new Date(ms).toLocaleTimeString(getLocale(), { hour: "numeric", minute: "2-digit" });
}

/** Focus the notch so a text field can take typing (it doesn't activate on its own). */
function focusNotch() {
	void getCurrentWebviewWindow().setFocus().catch(console.error);
}

function Agenda({
	events,
	configured,
	status
}: {
	events: CalendarEvent[];
	configured: boolean;
	status: CalendarStatus;
}) {
	const [now, setNow] = useState(() => Date.now());
	useEffect(() => {
		const timer = setInterval(() => setNow(Date.now()), 60_000);
		return () => clearInterval(timer);
	}, []);
	const days = useMemo(() => {
		const today = new Date(now);
		const tomorrow = new Date(today.getFullYear(), today.getMonth(), today.getDate() + 1);
		return [
			{ label: t("notch.calendar.agendaToday"), items: eventsOn(events, today) },
			{ label: t("notch.calendar.agendaTomorrow"), items: eventsOn(events, tomorrow) }
		];
	}, [events, now]);

	if (!configured) {
		return (
			<div className="agenda-setup">
				<p>{t("notch.calendar.agendaSetup")}</p>
				<button type="button" onClick={() => invoke("open_settings_window").catch(console.error)}>
					{t("notch.calendar.openSettings")}
				</button>
			</div>
		);
	}

	return (
		<div className="agenda-list">
			{status === "loading" && events.length === 0 && (
				<div className="agenda-empty">{t("notch.calendar.agendaLoading")}</div>
			)}
			{status === "error" && <div className="agenda-error">{t("notch.calendar.agendaError")}</div>}
			{days.map((day) => (
				<div key={day.label} className="agenda-day">
					<div className="agenda-day-label">{day.label}</div>
					{day.items.length === 0 ? (
						<div className="agenda-empty">{t("notch.calendar.agendaNothing")}</div>
					) : (
						day.items.map((event) => {
							const past = event.end <= now;
							const current = event.start <= now && event.end > now;
							return (
								<div
									key={event.id}
									className={`agenda-item${past ? " past" : ""}${current ? " current" : ""}`}
									title={event.location || undefined}
								>
									<span className="agenda-time">
										{event.allDay ? t("notch.calendar.allDay") : formatTime(event.start)}
									</span>
									<span className="agenda-title">{event.title || t("notch.calendar.untitled")}</span>
								</div>
							);
						})
					)}
				</div>
			))}
		</div>
	);
}

function Tasks() {
	const [tasks, setTasks] = useState<Task[]>(() => readTasks(localStorage.getItem("bloom-tasks")));
	const [draft, setDraft] = useState("");
	const inputRef = useRef<HTMLInputElement>(null);

	useSettingsSync({
		"bloom-tasks": { set: (value: unknown) => setTasks(readTasks(value)), defaultValue: "[]" }
	});

	const save = (next: Task[]) => {
		setTasks(next);
		const value = JSON.stringify(next);
		localStorage.setItem("bloom-tasks", value);
		invoke("save_setting", { key: "bloom-tasks", value }).catch(console.error);
	};

	const add = () => {
		const text = draft.trim();
		if (!text) return;
		save([{ id: `t${Date.now()}`, text, done: false }, ...tasks]);
		setDraft("");
	};

	const open = tasks.filter((task) => !task.done);
	const done = tasks.filter((task) => task.done);

	return (
		<div className="tasks">
			<div className="tasks-input">
				<input
					ref={inputRef}
					type="text"
					value={draft}
					placeholder={t("notch.calendar.taskPlaceholder")}
					aria-label={t("notch.calendar.newTask")}
					onMouseDown={focusNotch}
					onFocus={focusNotch}
					onChange={(e) => setDraft(e.target.value)}
					onKeyDown={(e) => {
						if (e.key === "Enter") add();
						if (e.key === "Escape") inputRef.current?.blur();
					}}
					onWheel={(e) => e.stopPropagation()}
				/>
				<button type="button" aria-label={t("notch.calendar.addTask")} onClick={add} disabled={!draft.trim()}>
					<Plus size={13} />
				</button>
			</div>
			<div className="tasks-list" onWheel={(e) => e.stopPropagation()}>
				{tasks.length === 0 && <div className="agenda-empty">{t("notch.calendar.noTasks")}</div>}
				{[...open, ...done].map((task) => (
					<div key={task.id} className={`task-item${task.done ? " done" : ""}`}>
						<label>
							<input
								type="checkbox"
								checked={task.done}
								onChange={() =>
									save(tasks.map((x) => (x.id === task.id ? { ...x, done: !x.done } : x)))
								}
							/>
							<span>{task.text}</span>
						</label>
						<button
							type="button"
							aria-label={t("notch.calendar.deleteTask", { task: task.text })}
							onClick={() => save(tasks.filter((x) => x.id !== task.id))}
						>
							<X size={11} />
						</button>
					</div>
				))}
			</div>
			{done.length > 0 && (
				<button type="button" className="tasks-clear" onClick={() => save(open)}>
					{t("notch.calendar.clearDone")}
				</button>
			)}
		</div>
	);
}

/**
 * The right side of the Calendar view: Bloom's timer, today's agenda from
 * the user's calendars, or a task list, picked with the switch on top.
 */
export function CalendarSide({
	timer,
	events,
	configured,
	status
}: {
	timer: ReactNode;
	events: CalendarEvent[];
	configured: boolean;
	status: CalendarStatus;
}) {
	useTranslation();
	const [view, setView] = useState<SideView>(() => {
		const saved = localStorage.getItem(VIEW_KEY);
		return saved === "today" || saved === "tasks" ? saved : "timer";
	});
	const choose = (next: SideView) => {
		setView(next);
		localStorage.setItem(VIEW_KEY, next);
	};
	const views: { id: SideView; label: string; icon: ReactNode }[] = [
		{ id: "timer", label: t("notch.calendar.viewTimer"), icon: <Timer size={12} /> },
		{ id: "today", label: t("notch.calendar.viewToday"), icon: <CalendarDays size={12} /> },
		{ id: "tasks", label: t("notch.calendar.viewTasks"), icon: <ListTodo size={12} /> }
	];

	return (
		<div className="calendar-side">
			<div className="calendar-side-switch" role="tablist">
				{views.map((v) => (
					<button
						key={v.id}
						type="button"
						role="tab"
						aria-selected={view === v.id}
						title={v.label}
						className={view === v.id ? "active" : ""}
						onClick={() => choose(v.id)}
					>
						{v.icon}
						<span>{v.label}</span>
					</button>
				))}
			</div>
			<div className="calendar-side-body">
				{view === "timer" && timer}
				{view === "today" && <Agenda events={events} configured={configured} status={status} />}
				{view === "tasks" && <Tasks />}
			</div>
		</div>
	);
}

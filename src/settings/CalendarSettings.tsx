import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { BellRing, CalendarDays, PauseCircle } from "lucide-react";
import { SettingRow } from "./SettingRow";
import { t, useTranslation } from "../i18n";

async function save(key: string, value: string) {
	try {
		await invoke("save_setting", { key, value });
		localStorage.setItem(key, value);
	} catch (error) {
		console.error(`Failed to save ${key}:`, error);
	}
}

/** iCal calendar addresses plus the reminder and pause-music switches. */
export function CalendarSettings() {
	useTranslation();
	const [urls, setUrls] = useState(localStorage.getItem("bloom-calendar-urls") ?? "");
	const [reminders, setReminders] = useState(localStorage.getItem("bloom-calendar-reminders") !== "false");
	const [pauseMusic, setPauseMusic] = useState(localStorage.getItem("bloom-calendar-pause-music") !== "false");

	return (
		<div className="setting-group">
			<SettingRow icon={CalendarDays} label={t("integrations.calendarAddresses")} desc={t("integrations.calendarAddressesDesc")} />
			<div className="calendar-urls">
				<textarea
					value={urls}
					rows={3}
					spellCheck={false}
					aria-label={t("integrations.calendarAddresses")}
					placeholder="https://calendar.google.com/calendar/ical/…/basic.ics"
					onChange={(e) => setUrls(e.target.value)}
					onBlur={() => {
						if (urls !== (localStorage.getItem("bloom-calendar-urls") ?? "")) save("bloom-calendar-urls", urls.trim());
					}}
				/>
				<p>{t("integrations.calendarHowGoogle")}</p>
				<p>{t("integrations.calendarHowOutlook")}</p>
			</div>
			<div className="setting-divider" />
			<SettingRow icon={BellRing} label={t("integrations.calendarReminders")} desc={t("integrations.calendarRemindersDesc")}>
				<label className="toggle-switch">
					<input
						type="checkbox"
						checked={reminders}
						onChange={() => {
							const next = !reminders;
							setReminders(next);
							save("bloom-calendar-reminders", String(next));
						}}
					/>
					<span className="slider"></span>
				</label>
			</SettingRow>
			<SettingRow
				icon={PauseCircle}
				label={t("integrations.calendarPauseMusic")}
				desc={t("integrations.calendarPauseMusicDesc")}
				divider={false}
			>
				<label className="toggle-switch">
					<input
						type="checkbox"
						checked={pauseMusic}
						onChange={() => {
							const next = !pauseMusic;
							setPauseMusic(next);
							save("bloom-calendar-pause-music", String(next));
						}}
					/>
					<span className="slider"></span>
				</label>
			</SettingRow>
		</div>
	);
}

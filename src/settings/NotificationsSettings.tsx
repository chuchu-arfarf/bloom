import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Bell, BellRing, CircleAlert, Settings2 } from "lucide-react";
import { SettingRow } from "./SettingRow";
import { t, useTranslation } from "../i18n";
import { MUTED_KEY, POPUPS_KEY, mutedApps, type NoticeApp, type NoticeList } from "../notifications/notices";

async function save(key: string, value: string) {
	try {
		await invoke("save_setting", { key, value });
		localStorage.setItem(key, value);
	} catch (error) {
		console.error(`Failed to save ${key}:`, error);
	}
}

/** Popups for Windows notifications on the Island, for all apps or some. */
export function NotificationsSettings() {
	useTranslation();
	const [popups, setPopups] = useState(localStorage.getItem(POPUPS_KEY) !== "false");
	const [muted, setMuted] = useState(mutedApps);
	const [apps, setApps] = useState<NoticeApp[]>([]);
	const [error, setError] = useState<string | null>(null);

	useEffect(() => {
		const refresh = () => {
			invoke<NoticeApp[]>("notifications_apps").then(setApps).catch(console.error);
			invoke<NoticeList>("notifications_list")
				.then((list) => setError(list.error))
				.catch(console.error);
		};
		refresh();
		const unlisten = listen<NoticeList>("notifications-changed", (e) => {
			setError(e.payload.error);
			invoke<NoticeApp[]>("notifications_apps").then(setApps).catch(console.error);
		});
		return () => {
			unlisten.then((fn) => fn());
		};
	}, []);

	const toggleApp = (id: string) => {
		const next = new Set(muted);
		if (next.has(id)) next.delete(id);
		else next.add(id);
		setMuted(next);
		save(MUTED_KEY, JSON.stringify([...next]));
	};

	return (
		<div className="setting-group">
			<SettingRow
				icon={BellRing}
				label={t("integrations.notificationsPopups")}
				desc={t("integrations.notificationsPopupsDesc")}
			>
				<label className="toggle-switch">
					<input
						type="checkbox"
						checked={popups}
						onChange={() => {
							const next = !popups;
							setPopups(next);
							save(POPUPS_KEY, String(next));
						}}
					/>
					<span className="slider"></span>
				</label>
			</SettingRow>
			{error && (
				<SettingRow
					icon={CircleAlert}
					label={t("integrations.notificationsUnavailable")}
					desc={error}
				/>
			)}
			{popups &&
				apps.map((app) => (
					<SettingRow
						key={app.id}
						icon={Bell}
						label={app.name}
						desc={t("integrations.notificationsAppDesc")}
					>
						<label className="toggle-switch">
							<input type="checkbox" checked={!muted.has(app.id)} onChange={() => toggleApp(app.id)} />
							<span className="slider"></span>
						</label>
					</SettingRow>
				))}
			<SettingRow
				icon={Settings2}
				label={t("integrations.notificationsWindows")}
				desc={t("integrations.notificationsWindowsDesc")}
				action
				divider={false}
				onClick={() =>
					invoke("open_app", { appName: "ms-settings:notifications" }).catch(console.error)
				}
			/>
		</div>
	);
}

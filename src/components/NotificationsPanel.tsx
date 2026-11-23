import { useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Bell, BellOff, X } from "lucide-react";
import { t, tCount, getLocale, useTranslation } from "../i18n";
import { useAppIcon, type Notice, type NoticeList } from "../notifications/notices";
import "./NotificationsPanel.css";

/** "now", "5m", "2h", or the date for anything older than a day. */
function age(at: number, now: number, locale: string): string {
	const minutes = Math.floor((now - at) / 60_000);
	if (minutes < 1) return t("notch.notifications.now");
	const format = new Intl.RelativeTimeFormat(locale, { style: "narrow", numeric: "always" });
	if (minutes < 60) return format.format(-minutes, "minute");
	const hours = Math.floor(minutes / 60);
	if (hours < 24) return format.format(-hours, "hour");
	return new Date(at).toLocaleDateString(locale, { month: "short", day: "numeric" });
}

export function AppIcon({ appId, size }: { appId: string; size: number }) {
	const icon = useAppIcon(appId);
	return icon ? (
		<img className="notice-app-icon" src={icon} width={size} height={size} alt="" draggable={false} />
	) : (
		<span className="notice-app-icon placeholder" style={{ width: size, height: size }} aria-hidden="true">
			<Bell size={Math.round(size * 0.55)} strokeWidth={2} />
		</span>
	);
}

interface Group {
	appId: string;
	app: string;
	items: Notice[];
}

/** Newest app first; each app's notifications newest first. */
function groupByApp(items: Notice[]): Group[] {
	const groups = new Map<string, Group>();
	for (const notice of items) {
		const group = groups.get(notice.app_id);
		if (group) group.items.push(notice);
		else groups.set(notice.app_id, { appId: notice.app_id, app: notice.app, items: [notice] });
	}
	return [...groups.values()];
}

/** The notification drawer: what is waiting in Windows' notification center.
 *  `height` is the whole open notch, including its 36px top strip. */
export function NotificationsPanel({ list, height }: { list: NoticeList; height: number }) {
	const { locale } = useTranslation();
	const groups = useMemo(() => groupByApp(list.items), [list.items]);
	const now = Date.now();
	const open = (id: number) => invoke("notification_open", { id }).catch(console.error);
	const dismiss = (id: number) => invoke("notification_dismiss", { id }).catch(console.error);
	const clear = (appId: string | null) =>
		invoke("notifications_clear", { appId }).catch(console.error);

	return (
		<div className="notices-panel" style={{ height: height - 36 }} onClick={(e) => e.stopPropagation()}>
			<div className="notices-header">
				<span className="notices-title">{t("notch.notifications.title")}</span>
				{list.items.length > 0 && (
					<button type="button" className="notices-clear-all" onClick={() => clear(null)}>
						{t("notch.notifications.clearAll")}
					</button>
				)}
			</div>
			{list.items.length === 0 ? (
				<div className="notices-empty">
					<BellOff size={20} strokeWidth={1.8} />
					<span>{list.error ? t("notch.notifications.unavailable") : t("notch.notifications.empty")}</span>
				</div>
			) : (
				<div className="notices-list">
					{groups.map((group) => (
						<section key={group.appId} className="notices-group">
							<div className="notices-group-head">
								<AppIcon appId={group.appId} size={16} />
								<span className="notices-group-name">{group.app}</span>
								{group.items.length > 1 && (
									<span className="notices-group-count">
										{tCount("notch.notifications.count", group.items.length)}
									</span>
								)}
								<button
									type="button"
									className="notices-icon-button"
									aria-label={t("notch.notifications.clearApp", { app: group.app })}
									title={t("notch.notifications.clearApp", { app: group.app })}
									onClick={() => clear(group.appId)}
								>
									<X size={12} strokeWidth={2.4} />
								</button>
							</div>
							{group.items.map((notice) => (
								<div
									key={notice.id}
									className="notice-card"
									role="button"
									tabIndex={0}
									title={t("notch.notifications.openApp", { app: notice.app })}
									onClick={() => open(notice.id)}
									onKeyDown={(e) => {
										if (e.key === "Enter") open(notice.id);
										if (e.key === "Delete") dismiss(notice.id);
									}}
								>
									<div className="notice-card-top">
										<span className="notice-card-title">{notice.title}</span>
										<span className="notice-card-age">{age(notice.at, now, locale ?? getLocale())}</span>
										<button
											type="button"
											className="notices-icon-button notice-dismiss"
											aria-label={t("notch.notifications.dismiss")}
											onClick={(e) => {
												e.stopPropagation();
												dismiss(notice.id);
											}}
										>
											<X size={11} strokeWidth={2.4} />
										</button>
									</div>
									{notice.body && <div className="notice-card-body">{notice.body}</div>}
								</div>
							))}
						</section>
					))}
				</div>
			)}
		</div>
	);
}

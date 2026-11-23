import { t, useTranslation } from "../i18n";
import {
	PanelTop,
	Calendar,
	BellRing,
	Music,
	Minimize2,
	LayoutList,
	Sparkles,
	Circle,
	CloudSun,
	Timer,
	Archive,
	Headphones,
	Bell,
	SlidersHorizontal,
	X
} from "lucide-react";
import { SettingRow } from "./SettingRow";
import { StatusWidgetConfig } from "../components/StatusWidgetConfig";
import type { WidgetConfig } from "./types";
import type { NotchPanelChoice } from "./useSettings";

/** The notch's panels, in the order the notch cycles through them. */
const PANEL_CHOICES: readonly (readonly [NotchPanelChoice, string])[] = [
	["calendar", "Calendar"],
	["command-center", "Command Center"],
	["devices", "Connected devices"],
	["notifications", "Notifications"]
];

interface NotchTabProps {
	notchMode: string;
	setNotchModeValue: (mode: string) => void;
	notchEdgeDelay: number;
	handleNotchEdgeDelayChange: (val: number) => void;
	calendarMondayFirst: boolean;
	toggleCalendarMondayFirst: () => void;
	calendarEnabled: boolean;
	toggleCalendar: () => void;
	controlsPanelEnabled: boolean;
	toggleControlsPanel: () => void;
	devicesPanelEnabled: boolean;
	toggleDevicesPanel: () => void;
	notificationsPanelEnabled: boolean;
	toggleNotificationsPanel: () => void;
	enabledPanelCount: number;
	notchRememberPanel: boolean;
	toggleNotchRememberPanel: () => void;
	notchDefaultPanel: NotchPanelChoice;
	changeNotchDefaultPanel: (panel: NotchPanelChoice) => void;
	timerSoundEnabled: boolean;
	toggleTimerSound: () => void;
	musicModeEnabled: boolean;
	toggleMusicMode: () => void;
	musicCompactNotch: boolean;
	toggleMusicCompactNotch: () => void;
	shelfPersist: boolean;
	toggleShelfPersist: () => void;
	mediaLayout: "classic" | "compact";
	toggleMediaLayout: (layout: "classic" | "compact") => void;
	mediaAmbienceEnabled: boolean;
	toggleAmbience: () => void;
	mediaCompactGlowEnabled: boolean;
	toggleCompactGlow: () => void;
	weatherEnabled: boolean;
	toggleWeather: () => void;
	tempUnitFahrenheit: boolean;
	toggleTempUnit: () => void;
	cityName: string;
	setCityName: (name: string) => void;
	citySearchResults: Array<{ name: string; country: string; latitude: number; longitude: number }>;
	showCityDropdown: boolean;
	setShowCityDropdown: (show: boolean) => void;
	selectCity: (city: {
		name: string;
		country: string;
		latitude: number;
		longitude: number;
	}) => void;
	handleCityClear: () => void;
	statusWidgets: WidgetConfig;
	handleWidgetsChange: (config: WidgetConfig) => void;
}

export function NotchTab({
	notchMode,
	setNotchModeValue,
	notchEdgeDelay,
	handleNotchEdgeDelayChange,
	calendarMondayFirst,
	toggleCalendarMondayFirst,
	calendarEnabled,
	toggleCalendar,
	controlsPanelEnabled,
	toggleControlsPanel,
	devicesPanelEnabled,
	toggleDevicesPanel,
	notificationsPanelEnabled,
	toggleNotificationsPanel,
	enabledPanelCount,
	notchRememberPanel,
	toggleNotchRememberPanel,
	notchDefaultPanel,
	changeNotchDefaultPanel,
	timerSoundEnabled,
	toggleTimerSound,
	musicModeEnabled,
	toggleMusicMode,
	musicCompactNotch,
	toggleMusicCompactNotch,
	shelfPersist,
	toggleShelfPersist,
	mediaLayout,
	toggleMediaLayout,
	mediaAmbienceEnabled,
	toggleAmbience,
	mediaCompactGlowEnabled,
	toggleCompactGlow,
	weatherEnabled,
	toggleWeather,
	tempUnitFahrenheit,
	toggleTempUnit,
	cityName,
	setCityName,
	citySearchResults,
	showCityDropdown,
	setShowCityDropdown,
	selectCity,
	handleCityClear,
	statusWidgets,
	handleWidgetsChange
}: NotchTabProps) {
	useTranslation();
	const panelOn: Record<NotchPanelChoice, boolean> = {
		calendar: calendarEnabled,
		"command-center": controlsPanelEnabled,
		devices: devicesPanelEnabled,
		notifications: notificationsPanelEnabled
	};
	const firstPanelOn = PANEL_CHOICES.find(([panel]) => panelOn[panel])?.[0] ?? "command-center";
	// The notch needs at least one panel, so the last one on stays on.
	const isLastPanel = (on: boolean) => on && enabledPanelCount <= 1;
	const lastPanelTitle = (on: boolean) =>
		isLastPanel(on) ? "The notch needs at least one panel" : undefined;

	return (
		<>
			<div className="setting-group-label">{t("settings.tabs.notch")}</div>
			<div className="setting-group">
				<SettingRow
					previewId="notch-behavior"
					icon={PanelTop}
					label={t("settings.notch.behavior")}
					desc={t("settings.notch.behaviorDesc")}
				>
					<select
						className="settings-select"
						value={notchMode}
						onChange={(e) => setNotchModeValue(e.target.value)}
					>
						<option value="fixed">{t("common.behavior.fixed")}</option>
						<option value="smart">{t("common.behavior.smart")}</option>
						<option value="peek">{t("common.behavior.peek")}</option>
					</select>
				</SettingRow>

				{notchMode !== "fixed" && (
					<SettingRow
						previewId="peek-delay"
						icon={Timer}
						label={t("settings.notch.peekDelay")}
						desc={
							notchEdgeDelay === 0
								? t("settings.notch.peekInstant")
								: t("ui.notchTab.restOnTheTopEdgeForNotchedgedelayMsToReveal", {
										notchEdgeDelay: notchEdgeDelay
									})
						}
					>
						<input
							type="range"
							min="0"
							max="800"
							step="50"
							value={notchEdgeDelay}
							onChange={(e) => handleNotchEdgeDelayChange(parseInt(e.target.value))}
							className="settings-slider"
						/>
					</SettingRow>
				)}
			</div>

			<div className="setting-group-label">{t("ui.notchTab.panels")}</div>
			<div className="setting-group">
				<SettingRow
					previewId="calendar-timer"
					icon={Calendar}
					label={t("settings.notch.calendar")}
					desc={t("settings.notch.calendarDesc")}
				>
					<label className="toggle-switch" title={lastPanelTitle(calendarEnabled)}>
						<input
							type="checkbox"
							checked={calendarEnabled}
							disabled={isLastPanel(calendarEnabled)}
							onChange={toggleCalendar}
						/>
						<span className="slider"></span>
					</label>
				</SettingRow>

				{calendarEnabled && (
					<SettingRow
						previewId="monday-first-calendar"
						icon={Calendar}
						label={t("ui.notchTab.mondayFirstCalendar")}
						desc={t("ui.notchTab.startEachWeekOnMondayInsteadOfSunday")}
					>
						<label className="toggle-switch">
							<input
								type="checkbox"
								checked={calendarMondayFirst}
								onChange={toggleCalendarMondayFirst}
							/>
							<span className="slider" />
						</label>
					</SettingRow>
				)}

				{calendarEnabled && (
					<SettingRow
						previewId="timer-sound"
						icon={BellRing}
						label={t("settings.notch.timerSound")}
						desc={t("settings.notch.timerSoundDesc")}
					>
						<label className="toggle-switch">
							<input type="checkbox" checked={timerSoundEnabled} onChange={toggleTimerSound} />
							<span className="slider"></span>
						</label>
					</SettingRow>
				)}

				<SettingRow
					previewId="command-center"
					icon={SlidersHorizontal}
					label={t("ui.notchTab.commandCenter")}
					desc={t("ui.notchTab.quickTogglesWiFiAndBluetooth")}
				>
					<label className="toggle-switch" title={lastPanelTitle(controlsPanelEnabled)}>
						<input
							type="checkbox"
							checked={controlsPanelEnabled}
							disabled={isLastPanel(controlsPanelEnabled)}
							onChange={toggleControlsPanel}
						/>
						<span className="slider"></span>
					</label>
				</SettingRow>

				<SettingRow
					previewId="connected-devices"
					icon={Headphones}
					label={t("ui.notchTab.connectedDevices")}
					desc={t("ui.notchTab.batteriesAndAudioOfYourConnectedDevices")}
				>
					<label className="toggle-switch" title={lastPanelTitle(devicesPanelEnabled)}>
						<input
							type="checkbox"
							checked={devicesPanelEnabled}
							disabled={isLastPanel(devicesPanelEnabled)}
							onChange={toggleDevicesPanel}
						/>
						<span className="slider"></span>
					</label>
				</SettingRow>

				<SettingRow
					icon={Bell}
					label={t("settings.notch.notificationsPanel")}
					desc={t("settings.notch.notificationsPanelDesc")}
				>
					<label className="toggle-switch" title={lastPanelTitle(notificationsPanelEnabled)}>
						<input
							type="checkbox"
							checked={notificationsPanelEnabled}
							disabled={isLastPanel(notificationsPanelEnabled)}
							onChange={toggleNotificationsPanel}
						/>
						<span className="slider"></span>
					</label>
				</SettingRow>

				<SettingRow
					previewId="remember-last-panel"
					icon={PanelTop}
					label={t("ui.notchTab.rememberLastPanel")}
					desc={t("ui.notchTab.reopenThePanelYouLastUsedTurnOffToChoose")}
				>
					<label className="toggle-switch">
						<input
							type="checkbox"
							checked={notchRememberPanel}
							onChange={toggleNotchRememberPanel}
						/>
						<span className="slider"></span>
					</label>
				</SettingRow>

				{!notchRememberPanel && (
					<SettingRow
						previewId="default-panel"
						icon={PanelTop}
						label={t("ui.notchTab.defaultPanel")}
						desc={t("ui.notchTab.openThisPanelWhenYouClickTheNotch")}
					>
						<select
							className="settings-select"
							value={panelOn[notchDefaultPanel] ? notchDefaultPanel : firstPanelOn}
							onChange={(e) => changeNotchDefaultPanel(e.target.value as NotchPanelChoice)}
						>
							{PANEL_CHOICES.map(([panel, label]) => (
								<option key={panel} value={panel} disabled={!panelOn[panel]}>
									{label}
								</option>
							))}
						</select>
					</SettingRow>
				)}
			</div>

			<div className="setting-group-label">{t("ui.dockExtras.music")}</div>
			<div className="setting-group">
				<SettingRow
					previewId="music-mode"
					icon={Music}
					label={t("settings.notch.music")}
					desc={t("settings.notch.musicDesc")}
				>
					<label className="toggle-switch">
						<input type="checkbox" checked={musicModeEnabled} onChange={toggleMusicMode} />
						<span className="slider"></span>
					</label>
				</SettingRow>

				{musicModeEnabled && (
					<details className="settings-disclosure">
						<summary>
							{t("ui.notchTab.musicOptions")}
							<span>{t("ui.notchTab.layoutAndVisualEffects")}</span>
						</summary>
						<SettingRow
							previewId="compact-mode"
							icon={Minimize2}
							label={t("settings.notch.compact")}
							desc={t("settings.notch.compactDesc")}
						>
							<label className="toggle-switch">
								<input
									type="checkbox"
									checked={musicCompactNotch}
									onChange={toggleMusicCompactNotch}
								/>
								<span className="slider"></span>
							</label>
						</SettingRow>

						<SettingRow
							previewId="media-layout"
							icon={LayoutList}
							label={t("settings.notch.mediaLayout")}
							desc={t("settings.notch.mediaLayoutDesc")}
						>
							<div className="unit-toggle-minimal wide">
								<span
									className={mediaLayout === "classic" ? "active" : ""}
									onClick={() => toggleMediaLayout("classic")}
								>
									{t("common.mediaLayout.classic")}
								</span>
								<span
									className={mediaLayout === "compact" ? "active" : ""}
									onClick={() => toggleMediaLayout("compact")}
								>
									{t("common.mediaLayout.compact")}
								</span>
							</div>
						</SettingRow>

						<SettingRow
							previewId="ambient-glow"
							icon={Sparkles}
							label={t("settings.notch.ambience")}
							desc={t("settings.notch.ambienceDesc")}
						>
							<label className="toggle-switch">
								<input type="checkbox" checked={mediaAmbienceEnabled} onChange={toggleAmbience} />
								<span className="slider"></span>
							</label>
						</SettingRow>

						<SettingRow
							previewId="compact-glow"
							icon={Circle}
							label={t("settings.notch.compactGlow")}
							desc={t("settings.notch.compactGlowDesc")}
							divider={false}
						>
							<label className="toggle-switch">
								<input
									type="checkbox"
									checked={mediaCompactGlowEnabled}
									onChange={toggleCompactGlow}
								/>
								<span className="slider"></span>
							</label>
						</SettingRow>
					</details>
				)}
			</div>

			<div className="setting-group-label">{t("ui.notchTab.shelf")}</div>
			<div className="setting-group">
				<SettingRow
					previewId="keep-items"
					icon={Archive}
					label={t("ui.notchTab.keepItems")}
					desc={t("ui.notchTab.keepShelfItemsAfterBloomCloses")}
					divider={false}
				>
					<label className="toggle-switch">
						<input type="checkbox" checked={shelfPersist} onChange={toggleShelfPersist} />
						<span className="slider"></span>
					</label>
				</SettingRow>
			</div>

			<div className="setting-group-label">{t("settings.groups.weather")}</div>
			<div className="setting-group">
				<SettingRow
					previewId="weather-status"
					icon={CloudSun}
					label={t("settings.notch.weather")}
					desc={cityName || t("settings.notch.weatherAuto")}
				>
					<div className="weather-controls">
						<div className="unit-toggle-minimal" onClick={toggleTempUnit}>
							<span className={!tempUnitFahrenheit ? "active" : ""}>{t("ui.app.c")}</span>
							<span className={tempUnitFahrenheit ? "active" : ""}>{t("ui.app.f")}</span>
						</div>
						<label className="toggle-switch">
							<input type="checkbox" checked={weatherEnabled} onChange={toggleWeather} />
							<span className="slider"></span>
						</label>
					</div>
				</SettingRow>

				{weatherEnabled && (
					<div className="manual-city-input">
						<div className="city-input-row">
							<input
								type="text"
								placeholder={t("settings.notch.cityPlaceholder")}
								value={cityName}
								onChange={(e) => setCityName(e.target.value)}
								onFocus={() => citySearchResults.length > 0 && setShowCityDropdown(true)}
								onKeyDown={(e) => {
									if (e.key === "Enter" && citySearchResults.length > 0) {
										e.preventDefault();
										selectCity(citySearchResults[0]);
									}
									if (e.key === "Escape") {
										setShowCityDropdown(false);
									}
								}}
								onBlur={() => setTimeout(() => setShowCityDropdown(false), 150)}
							/>
							{cityName && (
								<button
									className="city-clear-btn"
									onMouseDown={(e) => {
										e.preventDefault();
										handleCityClear();
									}}
									title={t("settings.notch.cityClear")}
								>
									<X size={10} strokeWidth={2.5} />
								</button>
							)}
						</div>
						{showCityDropdown && citySearchResults.length > 0 && (
							<div className="city-dropdown">
								{citySearchResults.map((city) => (
									<button
										key={`${city.name}-${city.latitude}`}
										className="city-dropdown-item"
										onMouseDown={(e) => {
											e.preventDefault();
											selectCity(city);
										}}
									>
										<span className="city-dropdown-name">{city.name}</span>
										<span className="city-dropdown-country">{city.country}</span>
									</button>
								))}
							</div>
						)}
					</div>
				)}
			</div>

			<div className="setting-group-label">{t("settings.groups.widgets")}</div>
			<div className="setting-group">
				<details className="settings-disclosure settings-disclosure-standalone">
					<summary>
						{t("ui.notchTab.chooseStatusWidgets")}
						<span>{t("ui.notchTab.arrangeWhatAppearsBesideTheClock")}</span>
					</summary>
					<StatusWidgetConfig value={statusWidgets} onChange={handleWidgetsChange} />
				</details>
			</div>
		</>
	);
}

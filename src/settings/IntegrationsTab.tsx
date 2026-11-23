import { AgentIntegrations } from "./AgentIntegrations";
import { ClaudeSettings } from "./ClaudeSettings";
import { CalendarSettings } from "./CalendarSettings";
import { NotificationsSettings } from "./NotificationsSettings";
import { t, useTranslation } from "../i18n";
import "./IntegrationsTab.css";

interface IntegrationsTabProps {
	agentAlertsEnabled: boolean;
	onToggleAgentAlerts: () => void;
}

export function IntegrationsTab({ agentAlertsEnabled, onToggleAgentAlerts }: IntegrationsTabProps) {
	useTranslation();
	return (
		<>
			<section className="integration-category" aria-labelledby="claude-panel-title">
				<div className="integration-category-heading">
					<h2 id="claude-panel-title">{t("claude.settings.title")}</h2>
					<p>{t("claude.settings.descriptionWeb")}</p>
				</div>
				<ClaudeSettings />
			</section>
			<section className="integration-category" aria-labelledby="agent-integrations-title">
				<div className="integration-category-heading">
					<h2 id="agent-integrations-title">{t("integrations.agents")}</h2>
					<p>{t("integrations.agentsDescription")}</p>
				</div>
				<AgentIntegrations enabled={agentAlertsEnabled} onToggle={onToggleAgentAlerts} />
			</section>
			<section className="integration-category" aria-labelledby="calendar-integrations-title">
				<div className="integration-category-heading">
					<h2 id="calendar-integrations-title">{t("integrations.calendars")}</h2>
					<p>{t("integrations.calendarsDescription")}</p>
				</div>
				<CalendarSettings />
			</section>
			<section className="integration-category" aria-labelledby="notification-settings-title">
				<div className="integration-category-heading">
					<h2 id="notification-settings-title">{t("integrations.notifications")}</h2>
					<p>{t("integrations.notificationsDescription")}</p>
				</div>
				<NotificationsSettings />
			</section>
		</>
	);
}

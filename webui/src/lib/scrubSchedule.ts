export type ScrubSchedulePreset = 'manual' | 'daily' | 'weekly' | 'monthly' | 'custom';

const PRESET_CRON: Record<Exclude<ScrubSchedulePreset, 'manual' | 'custom'>, string> = {
	daily: '0 2 * * *',
	weekly: '0 2 * * 0',
	monthly: '0 2 1 * *',
};

export function scrubSchedulePreset(schedule: string | null | undefined): ScrubSchedulePreset {
	const cron = schedule?.trim();
	if (!cron) return 'manual';
	if (cron === PRESET_CRON.daily) return 'daily';
	if (cron === PRESET_CRON.weekly) return 'weekly';
	if (cron === PRESET_CRON.monthly) return 'monthly';
	return 'custom';
}

export function scrubScheduleValue(preset: ScrubSchedulePreset, custom: string): string | null | undefined {
	if (preset === 'manual') return null;
	if (preset === 'custom') return custom.trim() || undefined;
	return PRESET_CRON[preset];
}

export function scrubScheduleSummary(schedule: string | null | undefined): string {
	const cron = schedule?.trim();
	switch (scrubSchedulePreset(cron)) {
		case 'manual': return 'Manual only';
		case 'daily': return 'Daily at 02:00 UTC';
		case 'weekly': return 'Weekly on Sunday at 02:00 UTC';
		case 'monthly': return 'Monthly on day 1 at 02:00 UTC';
		case 'custom': return `${cron} (UTC)`;
	}
}

export function formatUtcTimestamp(timestamp: string): string {
	const date = new Date(timestamp);
	if (Number.isNaN(date.getTime())) return `${timestamp} (UTC)`;
	return date.toISOString().replace('T', ' ').replace(/\.\d{3}Z$/, ' UTC');
}

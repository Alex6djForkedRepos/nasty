import { describe, expect, test } from 'vitest';
import {
	formatUtcTimestamp,
	scrubSchedulePreset,
	scrubScheduleSummary,
	scrubScheduleValue,
} from './scrubSchedule';

describe('scrub schedule presets', () => {
	test('recognizes manual and canonical schedules', () => {
		expect(scrubSchedulePreset(null)).toBe('manual');
		expect(scrubSchedulePreset('')).toBe('manual');
		expect(scrubSchedulePreset(' 0 2 * * * ')).toBe('daily');
		expect(scrubSchedulePreset('0 2 * * 0')).toBe('weekly');
		expect(scrubSchedulePreset('0 2 1 * *')).toBe('monthly');
		expect(scrubSchedulePreset('15 4 * * 2')).toBe('custom');
	});

	test('produces the API value, with only Manual mapping to null', () => {
		expect(scrubScheduleValue('manual', 'ignored')).toBeNull();
		expect(scrubScheduleValue('daily', '')).toBe('0 2 * * *');
		expect(scrubScheduleValue('weekly', '')).toBe('0 2 * * 0');
		expect(scrubScheduleValue('monthly', '')).toBe('0 2 1 * *');
		expect(scrubScheduleValue('custom', ' 15 4 * * 2 ')).toBe('15 4 * * 2');
		expect(scrubScheduleValue('custom', '')).toBeUndefined();
		expect(scrubScheduleValue('custom', '   ')).toBeUndefined();
	});
});

describe('scrub schedule display', () => {
	test('uses readable summaries for presets and raw cron for custom schedules', () => {
		expect(scrubScheduleSummary(null)).toBe('Manual only');
		expect(scrubScheduleSummary('0 2 * * *')).toBe('Daily at 02:00 UTC');
		expect(scrubScheduleSummary('0 2 * * 0')).toBe('Weekly on Sunday at 02:00 UTC');
		expect(scrubScheduleSummary('0 2 1 * *')).toBe('Monthly on day 1 at 02:00 UTC');
		expect(scrubScheduleSummary('15 4 * * 2')).toBe('15 4 * * 2 (UTC)');
	});

	test('formats backend timestamps as an explicit UTC value', () => {
		expect(formatUtcTimestamp('2026-09-15T02:00:00Z')).toBe('2026-09-15 02:00:00 UTC');
		expect(formatUtcTimestamp('2026-09-15T04:00:00+02:00')).toBe('2026-09-15 02:00:00 UTC');
	});
});

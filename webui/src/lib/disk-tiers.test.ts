import { describe, expect, it } from 'vitest';
import { diskTier, tierCapabilities } from './disk-tiers';

describe('disk tier classification', () => {
	it('uses media across SAS shelves and keeps NVMe as a separate SSD tier', () => {
		expect(diskTier({ media: 'ssd', native_interface: 'sas' })).toBe('ssd');
		expect(diskTier({ media: 'hdd', native_interface: 'sata' })).toBe('hdd');
		expect(diskTier({ media: 'ssd', native_interface: 'nvme' })).toBe('nvme');
		expect(diskTier({ media: null, native_interface: null })).toBe('unknown');
	});

	it('enables all tiers for SATA HDDs, SAS SSDs, and NVMe SSDs', () => {
		expect(tierCapabilities([
			{ media: 'hdd', native_interface: 'sata' },
			{ media: 'ssd', native_interface: 'sas' },
			{ media: 'ssd', native_interface: 'nvme' },
		])).toEqual({ hasNvme: true, hasSsd: true, hasHdd: true });
	});
});

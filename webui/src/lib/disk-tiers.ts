import type { BlockDevice } from './types';

/** Tiering is based on media, with NVMe as a distinct SSD tier. */
export function diskTier(device: Pick<BlockDevice, 'media' | 'native_interface'>): 'nvme' | 'ssd' | 'hdd' | 'unknown' {
	if (device.media === 'ssd' && device.native_interface === 'nvme') return 'nvme';
	return device.media ?? 'unknown';
}

export function tierCapabilities(devices: Pick<BlockDevice, 'media' | 'native_interface'>[]) {
	const tiers = new Set(devices.map(diskTier));
	return {
		hasNvme: tiers.has('nvme'),
		hasSsd: tiers.has('ssd'),
		hasHdd: tiers.has('hdd'),
	};
}

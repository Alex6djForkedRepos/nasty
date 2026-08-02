import { describe, expect, it } from 'vitest';
import {
	readSubvolumeCompressionPolicy,
	readSubvolumeStoragePolicy,
	storagePolicyUpdate,
} from './subvolume-storage-policy';

describe('subvolume storage policy', () => {
	it('distinguishes inherited compression from an explicit none override', () => {
		expect(readSubvolumeCompressionPolicy(undefined, undefined, undefined, {
			compression: 'zstd',
		})).toEqual({
			compression: '',
			effectiveCompression: 'zstd',
			inheritedCompression: 'zstd',
			inherited: true,
		});
		expect(readSubvolumeCompressionPolicy({ compression: 'none' }, { compression: 'none' }, undefined, {
			compression: 'zstd',
		})).toEqual({
			compression: 'none',
			effectiveCompression: 'none',
			inheritedCompression: 'zstd',
			inherited: false,
		});
	});

	it('uses filesystem defaults when no subvolume overrides exist', () => {
		expect(readSubvolumeStoragePolicy(undefined, undefined, undefined, {
			data_replicas: 2,
			erasure_code: true,
		})).toEqual({
			dataReplicas: '',
			erasureCode: 'inherit',
			effectiveDataReplicas: 2,
			effectiveErasureCode: true,
			inheritedDataReplicas: 2,
			inheritedErasureCode: true,
			configuredInheritedDataReplicas: 2,
			configuredInheritedErasureCode: true,
			dataReplicasInherited: true,
			erasureCodeInherited: true,
		});
	});

	it('recognizes explicit replica and erasure-code overrides', () => {
		expect(readSubvolumeStoragePolicy({
			data_replicas: '3',
			erasure_code: '0',
		}, {
			data_replicas: '3',
			erasure_code: '0',
		}, {
			data_replicas: '2',
			erasure_code: '1',
		}, {
			data_replicas: 1,
			erasure_code: true,
		})).toMatchObject({
			dataReplicas: '3',
			erasureCode: 'disabled',
			effectiveDataReplicas: 3,
			effectiveErasureCode: false,
			dataReplicasInherited: false,
			erasureCodeInherited: false,
		});
	});

	it('does not confuse inherited effective values with overrides', () => {
		expect(readSubvolumeStoragePolicy({
			data_replicas: '2',
			erasure_code: '1',
		}, undefined, undefined, {
			data_replicas: 2,
			erasure_code: true,
		})).toMatchObject({
			dataReplicas: '',
			erasureCode: 'inherit',
			effectiveDataReplicas: 2,
			effectiveErasureCode: true,
			dataReplicasInherited: true,
			erasureCodeInherited: true,
		});
	});

	it('uses the immediate parent policy as the inheritance baseline', () => {
		expect(readSubvolumeStoragePolicy({
			data_replicas: '3',
			erasure_code: '1',
		}, {
			data_replicas: '3',
			erasure_code: '1',
		}, {
			data_replicas: '2',
			erasure_code: '0',
		}, {
			data_replicas: 1,
			erasure_code: true,
		})).toMatchObject({
			inheritedDataReplicas: 2,
			inheritedErasureCode: false,
			configuredInheritedDataReplicas: 2,
			configuredInheritedErasureCode: false,
		});
	});

	it('applies bcachefs fixups to operationally effective values', () => {
		expect(readSubvolumeStoragePolicy({
			data_replicas: '1',
			erasure_code: '1',
		}, undefined, {
			data_replicas: '4',
			erasure_code: '1',
		}, undefined)).toMatchObject({
			effectiveDataReplicas: 1,
			effectiveErasureCode: false,
			inheritedDataReplicas: 3,
			inheritedErasureCode: true,
			configuredInheritedDataReplicas: 4,
			configuredInheritedErasureCode: true,
		});
	});

	it('builds updates that can restore both filesystem defaults', () => {
		expect(storagePolicyUpdate('', 'inherit')).toEqual({
			data_replicas: 0,
			erasure_code: 'inherit',
		});
		expect(storagePolicyUpdate('2', 'enabled')).toEqual({
			data_replicas: 2,
			erasure_code: 'enabled',
		});
		expect(storagePolicyUpdate('2', 'enabled', {
			dataReplicas: '2',
			erasureCode: 'inherit',
		})).toEqual({ erasure_code: 'enabled' });
	});
});

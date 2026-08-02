import type { FilesystemOptions } from '$lib/types';

export type ErasureCodeSetting = 'inherit' | 'enabled' | 'disabled';

export interface SubvolumeStoragePolicy {
	dataReplicas: string;
	erasureCode: ErasureCodeSetting;
	effectiveDataReplicas: number;
	effectiveErasureCode: boolean;
	inheritedDataReplicas: number;
	inheritedErasureCode: boolean;
	configuredInheritedDataReplicas: number;
	configuredInheritedErasureCode: boolean;
	dataReplicasInherited: boolean;
	erasureCodeInherited: boolean;
}

type StorageDefaults = Pick<FilesystemOptions, 'data_replicas' | 'erasure_code'>;
type CompressionDefaults = Pick<FilesystemOptions, 'compression'>;

export interface SubvolumeCompressionPolicy {
	compression: string;
	effectiveCompression: string;
	inheritedCompression: string;
	inherited: boolean;
}

function optionEnabled(value: string): boolean {
	return value === '1' || value === 'true' || value === 'yes' || value === 'on';
}

function applyBcachefsFixups(dataReplicas: number, erasureCode: boolean) {
	if (dataReplicas === 1) erasureCode = false;
	if (erasureCode) dataReplicas = Math.min(dataReplicas, 3);
	return { dataReplicas, erasureCode };
}

export function readSubvolumeCompressionPolicy(
	effectiveOptions: Record<string, string> | undefined,
	overrides: Record<string, string> | undefined,
	inheritedOptions: Record<string, string> | undefined,
	defaults: CompressionDefaults | null | undefined,
): SubvolumeCompressionPolicy {
	const override = overrides?.compression;
	const inheritedCompression = inheritedOptions?.compression || defaults?.compression || 'none';
	return {
		compression: override ?? '',
		effectiveCompression: effectiveOptions?.compression || inheritedCompression,
		inheritedCompression,
		inherited: override === undefined,
	};
}

export function readSubvolumeStoragePolicy(
	effectiveOptions: Record<string, string> | undefined,
	overrides: Record<string, string> | undefined,
	inheritedOptions: Record<string, string> | undefined,
	defaults: StorageDefaults | null | undefined,
): SubvolumeStoragePolicy {
	const dataReplicas = overrides?.data_replicas;
	const erasureCode = overrides?.erasure_code;
	const configuredInheritedDataReplicas = Number.parseInt(inheritedOptions?.data_replicas ?? '', 10)
		|| defaults?.data_replicas || 1;
	const configuredInheritedErasureCode = inheritedOptions?.erasure_code === undefined
		? (defaults?.erasure_code ?? false)
		: optionEnabled(inheritedOptions.erasure_code);
	const effective = applyBcachefsFixups(
		Number.parseInt(effectiveOptions?.data_replicas ?? '', 10) || configuredInheritedDataReplicas,
		effectiveOptions?.erasure_code === undefined
			? configuredInheritedErasureCode
			: optionEnabled(effectiveOptions.erasure_code),
	);
	const inherited = applyBcachefsFixups(
		configuredInheritedDataReplicas,
		configuredInheritedErasureCode,
	);
	return {
		dataReplicas: dataReplicas ?? '',
		erasureCode: erasureCode === undefined
			? 'inherit'
			: optionEnabled(erasureCode) ? 'enabled' : 'disabled',
		effectiveDataReplicas: effective.dataReplicas,
		effectiveErasureCode: effective.erasureCode,
		inheritedDataReplicas: inherited.dataReplicas,
		inheritedErasureCode: inherited.erasureCode,
		configuredInheritedDataReplicas,
		configuredInheritedErasureCode,
		dataReplicasInherited: dataReplicas === undefined,
		erasureCodeInherited: erasureCode === undefined,
	};
}

export function storagePolicyUpdate(
	dataReplicas: string,
	erasureCode: ErasureCodeSetting,
	previous?: Pick<SubvolumeStoragePolicy, 'dataReplicas' | 'erasureCode'>,
) {
	const update: { data_replicas?: number; erasure_code?: ErasureCodeSetting } = {};
	if (!previous || dataReplicas !== previous.dataReplicas) {
		update.data_replicas = dataReplicas === '' ? 0 : Number.parseInt(dataReplicas, 10);
	}
	if (!previous || erasureCode !== previous.erasureCode) {
		update.erasure_code = erasureCode;
	}
	return update;
}

export interface VersionSwitchInput {
	name: string;
	url: string;
	update: boolean;
}

export function buildBcachefsSyncInputs(
	inputs: readonly { name: string; url: string; update?: boolean }[],
	recommendedRef: string
): VersionSwitchInput[] | null {
	if (!recommendedRef || !inputs.some((input) => input.name === 'bcachefs-tools')) return null;

	return inputs.map((input) => ({
		name: input.name,
		url: input.name === 'bcachefs-tools'
			? `github:koverstreet/bcachefs-tools/${recommendedRef}`
			: input.url,
		update: input.name === 'bcachefs-tools' ? true : input.update ?? false,
	}));
}

const SHARE_COLLECTIONS = new Set([
	'share.nfs',
	'share.smb',
	'share.iscsi',
	'share.nvmeof',
]);

export function isShareCollection(collection: unknown): boolean {
	return typeof collection === 'string' && SHARE_COLLECTIONS.has(collection);
}

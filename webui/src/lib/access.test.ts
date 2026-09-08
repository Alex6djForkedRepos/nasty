import { describe, expect, test } from 'vitest';
import {
	canAccessAuthenticatedRoute,
	canMutateProtocol,
	hasRootEquivalentAccess,
	hasUnscopedMutationAccess,
	redirectForRole,
} from './access';

describe('authenticated route policy', () => {
	test('standard users can access only the portal route', () => {
		expect(canAccessAuthenticatedRoute('user', '/portal')).toBe(true);
		expect(canAccessAuthenticatedRoute('user', '/portal/files')).toBe(true);
		for (const path of ['/', '/files', '/users', '/menu', '/settings']) {
			expect(canAccessAuthenticatedRoute('user', path)).toBe(false);
			expect(redirectForRole('user', path)).toBe('/portal');
		}
	});

	test('management roles retain management routes and cannot open the portal', () => {
		for (const role of ['admin', 'operator', 'readonly']) {
			expect(canAccessAuthenticatedRoute(role, '/files')).toBe(true);
			expect(redirectForRole(role, '/files')).toBeNull();
			expect(redirectForRole(role, '/portal')).toBe('/');
		}
	});

	test('root-equivalent access requires an unscoped admin', () => {
		expect(hasRootEquivalentAccess('admin', false)).toBe(true);
		expect(hasRootEquivalentAccess('admin', true)).toBe(false);
		for (const role of ['operator', 'readonly', 'user'] as const) {
			expect(hasRootEquivalentAccess(role, false)).toBe(false);
			expect(hasRootEquivalentAccess(role, true)).toBe(false);
		}
	});

	test('global mutations allow only unscoped operators and admins', () => {
		for (const role of ['admin', 'operator'] as const) {
			expect(hasUnscopedMutationAccess(role, false)).toBe(true);
			expect(hasUnscopedMutationAccess(role, true)).toBe(false);
		}
		for (const role of ['readonly', 'user'] as const) {
			expect(hasUnscopedMutationAccess(role, false)).toBe(false);
		}
	});

	test('share protocol mutations allow unscoped operators and admins', () => {
		for (const role of ['admin', 'operator'] as const) {
			expect(canMutateProtocol(role, false, false)).toBe(true);
			expect(canMutateProtocol(role, true, false)).toBe(false);
		}
		for (const role of ['readonly', 'user'] as const) {
			expect(canMutateProtocol(role, false, false)).toBe(false);
		}
	});

	test('system service mutations require an unscoped admin', () => {
		expect(canMutateProtocol('admin', false, true)).toBe(true);
		expect(canMutateProtocol('admin', true, true)).toBe(false);
		for (const role of ['operator', 'readonly', 'user'] as const) {
			expect(canMutateProtocol(role, false, true)).toBe(false);
		}
	});
});

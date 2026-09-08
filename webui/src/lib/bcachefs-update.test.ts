import { describe, expect, test } from 'vitest';
import { buildBcachefsSyncInputs } from './bcachefs-update';

describe('bcachefs sync payload', () => {
	test('re-pins and refreshes only bcachefs-tools', () => {
		expect(buildBcachefsSyncInputs([
			{ name: 'nasty', url: 'github:nasty-project/nasty/main' },
			{ name: 'bcachefs-tools', url: 'github:koverstreet/bcachefs-tools/v1.39.4' },
			{ name: 'tailscale-nixpkgs', url: 'github:NixOS/nixpkgs/nixos-unstable' },
		], 'v1.39.5')).toEqual([
			{ name: 'nasty', url: 'github:nasty-project/nasty/main', update: false },
			{ name: 'bcachefs-tools', url: 'github:koverstreet/bcachefs-tools/v1.39.5', update: true },
			{ name: 'tailscale-nixpkgs', url: 'github:NixOS/nixpkgs/nixos-unstable', update: false },
		]);
	});

	test('rejects missing target input or recommendation', () => {
		expect(buildBcachefsSyncInputs([
			{ name: 'nasty', url: 'github:nasty-project/nasty/main' },
		], 'v1.39.5')).toBeNull();
		expect(buildBcachefsSyncInputs([
			{ name: 'bcachefs-tools', url: 'github:koverstreet/bcachefs-tools/v1.39.4' },
		], '')).toBeNull();
	});

	test('preserves other refresh selections from the Update page', () => {
		const inputs = buildBcachefsSyncInputs([
			{ name: 'nasty', url: 'github:nasty-project/nasty/main', update: true },
			{ name: 'bcachefs-tools', url: 'github:koverstreet/bcachefs-tools/v1.39.4', update: false },
		], 'v1.39.5');

		expect(inputs?.map((input) => [input.name, input.update])).toEqual([
			['nasty', true],
			['bcachefs-tools', true],
		]);
	});
});

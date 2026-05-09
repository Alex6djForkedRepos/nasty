import { describe, expect, it } from 'vitest';
import { promoteOrphanedMembers } from './network';
import type { BondConfig, BridgeConfig, InterfaceConfig, NetworkConfig } from './types';

function iface(name: string): InterfaceConfig {
	return {
		name,
		enabled: true,
		ipv4: { method: 'static', addresses: ['192.0.2.1/24'], gateway: null },
		ipv6: { method: 'disabled', addresses: [], gateway: null },
		mtu: null,
	};
}

function bridge(name: string, members: string[]): BridgeConfig {
	return {
		name,
		members,
		ipv4: { method: 'inherit', addresses: [], gateway: null },
		ipv6: { method: 'inherit', addresses: [], gateway: null },
		mtu: null,
	};
}

function bond(name: string, members: string[]): BondConfig {
	return {
		name,
		members,
		mode: 'lacp',
		ipv4: { method: 'dhcp', addresses: [], gateway: null },
		ipv6: { method: 'slaac', addresses: [], gateway: null },
		mtu: null,
	};
}

function emptyNet(over: Partial<NetworkConfig> = {}): NetworkConfig {
	return { interfaces: [], dns: [], bonds: [], vlans: [], bridges: [], ...over };
}

describe('promoteOrphanedMembers', () => {
	it('promotes a sole bridge member that has no standalone entry', () => {
		// The headline case: br0 has ens18 as a member, ens18 isn't in
		// interfaces[] (it was only known as a bridge port). When br0
		// is removed, ens18 must be promoted or it drops out of config
		// entirely and the engine has no profile to emit for it.
		const net = emptyNet({ bridges: [bridge('br0', ['ens18'])] });
		const result = promoteOrphanedMembers(net, { kind: 'bridge', name: 'br0' }, ['ens18']);
		expect(result).toHaveLength(1);
		expect(result[0]).toMatchObject({
			name: 'ens18',
			enabled: true,
			ipv4: { method: 'dhcp' },
			ipv6: { method: 'slaac' },
		});
	});

	it('does not duplicate a member that is already in interfaces', () => {
		// Some setups have the iface listed both as a standalone entry
		// (carrying its own L3 from a prior topology) and as a bridge
		// member. Removing the bridge mustn't add a second entry.
		const net = emptyNet({
			interfaces: [iface('ens18')],
			bridges: [bridge('br0', ['ens18'])],
		});
		const result = promoteOrphanedMembers(net, { kind: 'bridge', name: 'br0' }, ['ens18']);
		expect(result).toHaveLength(1);
		// The pre-existing entry is preserved untouched (incl. its
		// static IP — we don't overwrite user config).
		expect(result[0].ipv4.method).toBe('static');
	});

	it('does not promote a member still claimed by another bridge', () => {
		// Edge case: shared-port topology where eth0 is a member of
		// both br0 and br1 (unusual but representable in our schema).
		// Removing br0 leaves eth0 still under br1, so don't promote.
		const net = emptyNet({
			bridges: [bridge('br0', ['eth0']), bridge('br1', ['eth0'])],
		});
		const result = promoteOrphanedMembers(net, { kind: 'bridge', name: 'br0' }, ['eth0']);
		expect(result).toHaveLength(0);
	});

	it('does not promote a member still claimed by a bond', () => {
		// eth0 is in both bridge br0 and bond bond0. Removing the
		// bridge keeps it inside the bond — no promotion.
		const net = emptyNet({
			bridges: [bridge('br0', ['eth0'])],
			bonds: [bond('bond0', ['eth0'])],
		});
		const result = promoteOrphanedMembers(net, { kind: 'bridge', name: 'br0' }, ['eth0']);
		expect(result).toHaveLength(0);
	});

	it('ignores the master being removed when scanning for other claims', () => {
		// The function is given the master's identity so it can ignore
		// it when checking "is this iface still mastered elsewhere".
		// Without this filter, the function would see eth0 still
		// listed under br0 and skip the promotion.
		const net = emptyNet({ bridges: [bridge('br0', ['eth0'])] });
		const result = promoteOrphanedMembers(net, { kind: 'bridge', name: 'br0' }, ['eth0']);
		expect(result.map((i) => i.name)).toEqual(['eth0']);
	});

	it('promotes multiple orphans from one master', () => {
		const net = emptyNet({ bonds: [bond('bond0', ['eth0', 'eth1', 'eth2'])] });
		const result = promoteOrphanedMembers(
			net,
			{ kind: 'bond', name: 'bond0' },
			['eth0', 'eth1', 'eth2'],
		);
		expect(result.map((i) => i.name)).toEqual(['eth0', 'eth1', 'eth2']);
		// All get the same DHCP defaults — we don't pick a "primary".
		expect(result.every((i) => i.ipv4.method === 'dhcp')).toBe(true);
	});

	it('handles bond removal symmetrically to bridge removal', () => {
		const net = emptyNet({ bonds: [bond('bond0', ['eth0'])] });
		const result = promoteOrphanedMembers(net, { kind: 'bond', name: 'bond0' }, ['eth0']);
		expect(result.map((i) => i.name)).toEqual(['eth0']);
	});
});

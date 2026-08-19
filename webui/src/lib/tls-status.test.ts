import { describe, expect, it } from 'vitest';
import { selectPrimaryTlsStatus, type AcmeStatus, type HostTlsStatus } from './tls-status';

const staleStatus: AcmeStatus = {
	state: 'success',
	message: 'Certificate active for nas.0f.ee',
	domain: 'nas.0f.ee',
	issuer: 'E7',
	issued: 'Wed, 20 May 2026 11:38:36 +0000',
	expires: 'Tue, 18 Aug 2026 11:38:35 +0000'
};

const renewedHost: HostTlsStatus = {
	host: 'nas.0f.ee',
	state: 'active',
	issuer: 'YE1',
	issued: 'Wed, 19 Aug 2026 00:54:50 +0000',
	expires: 'Tue, 17 Nov 2026 00:54:49 +0000'
};

describe('selectPrimaryTlsStatus', () => {
	it('uses the live primary-domain certificate instead of a stale ACME snapshot', () => {
		const selected = selectPrimaryTlsStatus('NAS.0F.EE', staleStatus, [renewedHost]);

		expect(selected).toMatchObject({
			state: 'success',
			domain: 'nas.0f.ee',
			issuer: 'YE1',
			issued: renewedHost.issued,
			expires: renewedHost.expires
		});
	});

	it('does not use another managed hostname for the primary card', () => {
		const selected = selectPrimaryTlsStatus('nas.0f.ee', staleStatus, [
			{ ...renewedHost, host: 'haze.0f.ee' }
		]);

		expect(selected).toBe(staleStatus);
	});

	it('keeps provisioning errors until a primary certificate exists', () => {
		const error: AcmeStatus = {
			state: 'error',
			message: 'DNS challenge failed',
			domain: 'nas.0f.ee'
		};

		expect(
			selectPrimaryTlsStatus('nas.0f.ee', error, [
				{ host: 'nas.0f.ee', state: 'pending' }
			])
		).toBe(error);
	});

	it('uses the live certificate when the operation snapshot is stale', () => {
		const running: AcmeStatus = {
			...staleStatus,
			state: 'running',
			message: 'Provisioning replacement certificate'
		};

		expect(selectPrimaryTlsStatus('nas.0f.ee', running, [renewedHost])).toMatchObject({
			state: 'success',
			issued: renewedHost.issued
		});

		const error: AcmeStatus = {
			...staleStatus,
			state: 'error',
			message: 'Replacement failed',
			issued: renewedHost.issued
		};
		expect(selectPrimaryTlsStatus('nas.0f.ee', error, [renewedHost])).toMatchObject({
			state: 'success',
			issued: renewedHost.issued
		});
	});

	it('ignores an ACME snapshot for a previously configured domain', () => {
		expect(selectPrimaryTlsStatus('new.0f.ee', staleStatus, [])).toBeNull();
		expect(
			selectPrimaryTlsStatus('new.0f.ee', staleStatus, [
				{ host: 'new.0f.ee', state: 'pending' }
			])
		).toMatchObject({ state: 'running', domain: 'new.0f.ee' });
	});

	it('uses live operation state if the ACME status request failed', () => {
		expect(
			selectPrimaryTlsStatus('nas.0f.ee', null, [
				{ host: 'nas.0f.ee', state: 'failed', renewal_error: 'DNS challenge failed' }
			])
		).toMatchObject({ state: 'error', message: 'DNS challenge failed' });

		expect(
			selectPrimaryTlsStatus('nas.0f.ee', null, [
				{ host: 'nas.0f.ee', state: 'issuing' }
			])
		).toMatchObject({ state: 'running' });
	});

	it('uses a live failure instead of a stale running snapshot', () => {
		const running: AcmeStatus = {
			state: 'running',
			message: 'Waiting for Caddy',
			domain: 'nas.0f.ee'
		};

		expect(
			selectPrimaryTlsStatus('nas.0f.ee', running, [
				{ host: 'nas.0f.ee', state: 'failed', renewal_error: 'DNS challenge failed' }
			])
		).toMatchObject({ state: 'error', message: 'DNS challenge failed' });
	});

	it('does not let stale success override a pending live host', () => {
		expect(
			selectPrimaryTlsStatus('nas.0f.ee', staleStatus, [
				{ host: 'nas.0f.ee', state: 'pending' }
			])
		).toMatchObject({ state: 'running', domain: 'nas.0f.ee' });
	});

	it('handles Caddy success before certificate metadata reaches disk', () => {
		expect(
			selectPrimaryTlsStatus('nas.0f.ee', staleStatus, [
				{ host: 'nas.0f.ee', state: 'active', message: 'certificate obtained successfully' }
			])
		).toMatchObject({ state: 'success', domain: 'nas.0f.ee' });
	});

	it('surfaces an expired live primary certificate as an error', () => {
		const selected = selectPrimaryTlsStatus('nas.0f.ee', staleStatus, [
			{ ...renewedHost, state: 'expired' }
		]);

		expect(selected?.state).toBe('error');
		expect(selected?.message).toContain('expired');
		expect(selected?.expires).toBe(renewedHost.expires);
	});
});

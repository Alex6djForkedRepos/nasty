export type AcmeStatus = {
	state: string;
	message: string;
	domain?: string;
	expires?: string;
	issued?: string;
	issuer?: string;
	last_attempt?: string;
};

export type HostTlsStatus = {
	host: string;
	state: 'active' | 'expiring' | 'expired' | 'issuing' | 'failed' | 'pending';
	issuer?: string;
	issued?: string;
	expires?: string;
	expires_in_days?: number;
	message?: string;
	renewal_error?: string;
	app?: string;
};

export function selectPrimaryTlsStatus(
	domain: string,
	acmeStatus: AcmeStatus | null,
	hostStatuses: HostTlsStatus[]
): AcmeStatus | null {
	const normalizedDomain = domain.trim().toLowerCase();
	const scopedAcmeStatus =
		acmeStatus?.domain && acmeStatus.domain.toLowerCase() !== normalizedDomain
			? null
			: acmeStatus;
	const hostStatus = hostStatuses.find((status) => status.host.toLowerCase() === normalizedDomain);
	if (!hostStatus) {
		return scopedAcmeStatus;
	}
	const hasCertificate = Boolean(hostStatus.issued || hostStatus.expires || hostStatus.issuer);
	if (!hasCertificate) {
		if (hostStatus.state === 'active') {
			return {
				state: 'success',
				message: hostStatus.message ?? `Certificate active for ${hostStatus.host}`,
				domain: hostStatus.host
			};
		}
		if (hostStatus.state === 'failed') {
			return {
				state: 'error',
				message: hostStatus.renewal_error ?? hostStatus.message ?? 'Certificate issuance failed',
				domain: hostStatus.host
			};
		}
		if (hostStatus.state === 'issuing' || hostStatus.state === 'pending') {
			if (
				hostStatus.state === 'pending' &&
				(scopedAcmeStatus?.state === 'running' || scopedAcmeStatus?.state === 'error')
			) {
				return scopedAcmeStatus;
			}
			return {
				state: 'running',
				message: hostStatus.message ?? 'Waiting for Caddy to obtain a certificate...',
				domain: hostStatus.host
			};
		}
		return null;
	}

	const expired = hostStatus.state === 'expired';
	return {
		state: expired ? 'error' : 'success',
		message: expired
			? `Certificate for ${hostStatus.host} expired${hostStatus.expires ? ` on ${hostStatus.expires}` : ''}`
			: `Certificate active for ${hostStatus.host}`,
		domain: hostStatus.host,
		issuer: hostStatus.issuer,
		issued: hostStatus.issued,
		expires: hostStatus.expires,
		last_attempt: scopedAcmeStatus?.last_attempt
	};
}

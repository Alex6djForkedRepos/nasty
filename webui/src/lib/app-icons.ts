import {
	siAdguard,
	siCaddy,
	siEmby,
	siGitea,
	siGrafana,
	siHomeassistant,
	siImmich,
	siInfluxdb,
	siJellyfin,
	siMariadb,
	siMinio,
	siMysql,
	siNextcloud,
	siNginx,
	siPaperlessngx,
	siPihole,
	siPlex,
	siPortainer,
	siPostgresql,
	siPrometheus,
	siQbittorrent,
	siRadarr,
	siRedis,
	siSonarr,
	siTraefikproxy,
	siVaultwarden,
	type SimpleIcon
} from 'simple-icons';
import type { App } from './types';

const iconAliases = new Map<string, SimpleIcon>([
	['adguard', siAdguard],
	['adguardhome', siAdguard],
	['adguard/adguardhome', siAdguard],
	['caddy', siCaddy],
	['emby', siEmby],
	['emby/embyserver', siEmby],
	['gitea', siGitea],
	['gitea/gitea', siGitea],
	['grafana', siGrafana],
	['grafana/grafana', siGrafana],
	['grafana/grafana-enterprise', siGrafana],
	['grafana/grafana-oss', siGrafana],
	['home-assistant', siHomeassistant],
	['homeassistant', siHomeassistant],
	['home-assistant/home-assistant', siHomeassistant],
	['immich', siImmich],
	['immich-server', siImmich],
	['immich-app/immich-server', siImmich],
	['influxdb', siInfluxdb],
	['influxdata/influxdb', siInfluxdb],
	['jellyfin', siJellyfin],
	['jellyfin/jellyfin', siJellyfin],
	['linuxserver/jellyfin', siJellyfin],
	['mariadb', siMariadb],
	['minio', siMinio],
	['minio/minio', siMinio],
	['mysql', siMysql],
	['nextcloud', siNextcloud],
	['nginx', siNginx],
	['nginxinc/nginx-unprivileged', siNginx],
	['paperless', siPaperlessngx],
	['paperless-ngx', siPaperlessngx],
	['paperless-ngx/paperless-ngx', siPaperlessngx],
	['pihole', siPihole],
	['pihole/pihole', siPihole],
	['plex', siPlex],
	['plexinc/pms-docker', siPlex],
	['linuxserver/plex', siPlex],
	['pms-docker', siPlex],
	['portainer', siPortainer],
	['portainer/portainer-ce', siPortainer],
	['postgres', siPostgresql],
	['postgresql', siPostgresql],
	['prometheus', siPrometheus],
	['prom/prometheus', siPrometheus],
	['qbittorrent', siQbittorrent],
	['linuxserver/qbittorrent', siQbittorrent],
	['radarr', siRadarr],
	['linuxserver/radarr', siRadarr],
	['redis', siRedis],
	['redis/redis-stack', siRedis],
	['redis/redis-stack-server', siRedis],
	['sonarr', siSonarr],
	['linuxserver/sonarr', siSonarr],
	['traefik', siTraefikproxy],
	['traefikproxy/traefik', siTraefikproxy],
	['vaultwarden', siVaultwarden],
	['vaultwarden/server', siVaultwarden]
]);

function normalizeAlias(value: string): string {
	return value.trim().toLowerCase().replace(/[_\s]+/g, '-');
}

/** Strip registry, tag, and digest while retaining the repository namespace. */
export function normalizeImageRepository(image: string): string {
	let value = image.trim().toLowerCase();
	if (!value || value.includes('://')) return '';

	value = value.split('@', 1)[0];
	const slash = value.lastIndexOf('/');
	const colon = value.lastIndexOf(':');
	if (colon > slash) value = value.slice(0, colon);

	const parts = value.split('/').filter(Boolean);
	if (parts.length > 1 && (parts[0].includes('.') || parts[0].includes(':') || parts[0] === 'localhost')) {
		parts.shift();
	}
	if (parts[0] === 'library') parts.shift();
	return parts.map(normalizeAlias).join('/');
}

function iconForImage(image: string): SimpleIcon | null {
	const repository = normalizeImageRepository(image);
	if (!repository) return null;
	return iconAliases.get(repository) ?? iconAliases.get(repository.split('/').at(-1) ?? '') ?? null;
}

export function resolveAppIcon(app: Pick<App, 'name' | 'image' | 'kind' | 'containers'>): SimpleIcon | null {
	const byName = iconAliases.get(normalizeAlias(app.name));
	if (app.kind === 'compose' && byName) return byName;

	const byPrimaryImage = iconForImage(app.image);
	if (app.kind !== 'compose') return byPrimaryImage ?? byName ?? null;

	const composeIcons = new Map<string, SimpleIcon>();
	if (byPrimaryImage) composeIcons.set(byPrimaryImage.slug, byPrimaryImage);
	for (const container of app.containers ?? []) {
		const icon = iconForImage(container.image);
		if (icon) composeIcons.set(icon.slug, icon);
	}
	return composeIcons.size === 1 ? composeIcons.values().next().value ?? null : null;
}

export function appMonogram(name: string): { initials: string; hue: number } {
	const normalized = name.trim();
	const parts = normalized.split(/[^a-z0-9]+/i).filter(Boolean);
	const initials = parts.length > 1
		? `${parts[0][0]}${parts[1][0]}`.toUpperCase()
		: (parts[0] || '?').slice(0, 2).toUpperCase();
	let hash = 0;
	for (const character of normalized) hash = (hash * 31 + character.charCodeAt(0)) >>> 0;
	return { initials, hue: hash % 360 };
}

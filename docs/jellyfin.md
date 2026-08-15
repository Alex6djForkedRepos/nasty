# Jellyfin on NASty

Jellyfin runs on NASty as a Docker Compose app. No NixOS configuration is
required. The setup below keeps Jellyfin's database on NASty's relocatable
appdata storage and gives it read-only access to the media library.

This guide starts with CPU transcoding and direct play. Hardware acceleration
is optional and covered separately because it grants the container access to a
host device.

## Before you start

You need:

- A mounted NASty filesystem containing your media.
- Apps enabled on a mounted filesystem.
- The media path as NASty sees it, for example `/fs/tank/media`.

If Apps are not enabled, open **Apps**, click **Install App**, select a storage
filesystem, and click **Enable Apps**. Enabling Apps creates the stable
`/appdata` location used below.

Use the narrowest useful media path. Mounting `/fs/tank/media` is safer than
giving the container access to all of `/fs/tank`.

## Deploy Jellyfin

1. Open **Apps** and click **Install App**.
2. Select **Compose**.
3. Set **App Name** to `jellyfin`.
4. Replace `MEDIA_FILESYSTEM` and `MEDIA_DIRECTORY` in the Compose file below.
   For media stored at `/fs/tank/media`, use `tank` and `media`.
5. Paste the Compose file and click **Deploy**.

```yaml
services:
  jellyfin:
    image: jellyfin/jellyfin:10.11
    ports:
      - "8096:8096/tcp"
      - "7359:7359/udp"
    volumes:
      - type: bind
        source: /appdata/jellyfin/config
        target: /config
      - type: bind
        source: /appdata/jellyfin/cache
        target: /cache
      - type: bind
        source: /fs/MEDIA_FILESYSTEM/MEDIA_DIRECTORY
        target: /media
        read_only: true
    restart: unless-stopped
    stop_grace_period: 30s
```

Leave **Allow unsafe** off. It is not needed for CPU transcoding or direct
play. The deployment validates the Compose file, creates the appdata
directories, pulls the official Jellyfin image, and opens the published app
ports in NASty's firewall.

Port `7359/UDP` enables Jellyfin client discovery on many local networks. It is
optional; remove that line if clients will always be configured with the
server address manually.

## Open and configure Jellyfin

Open `http://NASTY_IP:8096` from another computer on the LAN, or click the
`:8096` link in the Jellyfin row on the **Apps** page.

Complete Jellyfin's setup wizard. When adding a library, select `/media`.
Jellyfin can scan and stream the files but cannot modify or delete them because
the media mount is read-only. Jellyfin's database, artwork, logs, and cache are
stored under `/appdata/jellyfin`. Leave Jellyfin's automatic port mapping
disabled; NASty already controls how the service is exposed.

### HTTPS and a subdomain

Jellyfin should not use NASty's default `/apps/jellyfin/` path-prefix address;
Jellyfin emits absolute paths and expects to be served from a URL root. Use
either the direct `:8096` address or a subdomain:

1. Create DNS for a name such as `jellyfin.example.com` pointing to NASty.
2. On the **Apps** page, open Jellyfin's `...` menu and select **Subdomain...**.
3. Enter the fully qualified hostname, keep port `8096` as the upstream, and
   click **Save**.
4. Open `https://jellyfin.example.com`.

NASty's Caddy service provides the reverse proxy and uses the appliance's
existing TLS/ACME configuration. For remote access without exposing a public
service, use NASty's Tailscale support and connect to port `8096` over the
tailnet. Do not forward an unencrypted port `8096` directly to the internet.

If auto-discovered clients receive the wrong address, add the public server
URL to the Jellyfin service and redeploy:

```yaml
    environment:
      JELLYFIN_PublishedServerUrl: https://jellyfin.example.com
```

Set it to the address clients can actually reach. It is not required when
clients are configured manually.

## Multiple media directories

Add another read-only bind for each library when the files do not share a
common parent:

```yaml
      - type: bind
        source: /fs/tank/movies
        target: /media/movies
        read_only: true
      - type: bind
        source: /fs/archive/tv
        target: /media/tv
        read_only: true
```

Then select `/media/movies` and `/media/tv` in Jellyfin. Every referenced
filesystem must be mounted before deploying or starting the app.

## Hardware acceleration

Direct play does not use the server GPU. Try the baseline setup first and add
hardware acceleration only when Jellyfin must transcode media for a client.

### Intel or AMD `/dev/dri`

If the NASty host has `/dev/dri`, add this to the Jellyfin service:

```yaml
    devices:
      - /dev/dri:/dev/dri
```

Then enable **Allow unsafe** for the Compose app and deploy the change. Device
mapping intentionally requires this acknowledgement. Do not enable it if the
Apps editor reports that `/dev/dri` is missing.

In Jellyfin, open **Dashboard > Playback > Transcoding** and select the method
appropriate for the GPU, normally Intel Quick Sync or VA-API. The exact codec
support depends on the GPU generation. Test playback that actually requires a
transcode and check the Jellyfin logs; successful direct play does not prove
that hardware transcoding works.

NVIDIA acceleration is not covered by this recipe. It also requires compatible
host drivers and the NVIDIA Container Toolkit, which NASty does not currently
configure as part of Apps.

### DLNA

Jellyfin's DLNA server requires host networking. NASty treats host networking
as unsafe, and it does not combine cleanly with the normal published-port and
reverse-proxy workflow. Prefer native Jellyfin clients or configure clients
manually. Enable host networking only if DLNA is a firm requirement and you
understand the reduced isolation.

## Updates

The `10.11` image tag follows Jellyfin `10.11.x` patch releases but does not
change to a new minor release automatically. NASty never updates this app in
the background.

To install a new `10.11.x` image:

1. Back up `/appdata/jellyfin` as described below.
2. Open Jellyfin's `...` menu on the **Apps** page.
3. Select **Pull image**.
4. Wait for NASty to pull the image and recreate the container.
5. Open Jellyfin and verify the server version and library playback.

For a new Jellyfin minor release, read Jellyfin's release notes first. Then use
**Edit** on the Compose app, change the image tag deliberately, and deploy the
update. Pin a full version such as `10.11.11` instead if reproducibility is more
important than receiving patch releases through **Pull image**.

## Backup and recovery

The configuration and Compose definition are separate:

- `/appdata/jellyfin` contains the Jellyfin database, settings, artwork, and
  cache.
- `/var/lib/nasty/apps/jellyfin` contains NASty's Compose definition and is
  included in a NASty system-recovery backup.
- The media remains in the `/fs/...` paths selected above.

For a consistent Jellyfin database backup:

1. Stop Jellyfin from the **Apps** page.
2. Open the Appdata details on that page and note where `/appdata` currently
   lives, for example `/fs/tank/appdata`.
3. Include `/fs/tank/appdata/jellyfin` in a backup profile, or snapshot/back up
   the complete `appdata` subvolume.
4. Start Jellyfin again.

Update the backup source if appdata is later relocated to another filesystem.
Back up the media separately according to its value and replaceability.

To recover, restore the Jellyfin appdata directory, deploy the same Compose
definition and image version, and then start Jellyfin. Removing the Compose app
does not delete the bind-mounted `/appdata/jellyfin` directory, but it should
never be treated as a substitute for a backup.

## Troubleshooting

### Jellyfin does not start

Open **Apps → Jellyfin → Logs**. The most common causes are an invalid media
path, an unmounted filesystem, or permissions on an existing directory.

### Port 8096 is already in use

Change only the host side of the mapping, for example:

```yaml
      - "8097:8096/tcp"
```

Then use `http://NASTY_IP:8097`. If using a subdomain, select host port `8097`
as its upstream.

### The direct URL works but the subdomain does not

Confirm that the hostname resolves to NASty, the subdomain is configured on
the Jellyfin app, and port `8096` is selected as the reverse-proxy upstream.
Do not use the `/apps/jellyfin/` path-prefix URL.

### Media is missing in Jellyfin

Confirm that the Compose source path exists in NASty's file browser and that
the corresponding filesystem is mounted. Container paths are different from
host paths: a source such as `/fs/tank/media` appears as `/media` inside
Jellyfin.

## Upstream documentation

- [Jellyfin container installation](https://jellyfin.org/docs/general/installation/container/)
- [Jellyfin setup wizard](https://jellyfin.org/docs/general/post-install/setup-wizard/)
- [Jellyfin hardware acceleration](https://jellyfin.org/docs/general/post-install/transcoding/hardware-acceleration/)

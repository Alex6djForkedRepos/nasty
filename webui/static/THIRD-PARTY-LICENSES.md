# Third-Party Licenses

NASty is licensed under GPL-3.0-only. This document lists the licenses of
third-party components used by NASty.

All dependencies use licenses compatible with GPL-3.0.

## Rust Engine Dependencies

| Crate | License |
|---|---|
| anyhow | MIT OR Apache-2.0 |
| argon2 | MIT OR Apache-2.0 |
| axum | MIT |
| base64 | MIT OR Apache-2.0 |
| futures-util | MIT OR Apache-2.0 |
| libc | MIT OR Apache-2.0 |
| portable-pty | MIT |
| rand | MIT OR Apache-2.0 |
| reqwest | MIT OR Apache-2.0 |
| rnix | MIT |
| rowan | MIT OR Apache-2.0 |
| rusqlite | MIT |
| schemars | MIT |
| serde | MIT OR Apache-2.0 |
| serde_json | MIT OR Apache-2.0 |
| sha2 | MIT OR Apache-2.0 |
| thiserror | MIT OR Apache-2.0 |
| tokio | MIT |
| tokio-util | MIT |
| tracing | MIT |
| tracing-subscriber | MIT |
| uuid | MIT OR Apache-2.0 |
| xattr | MIT OR Apache-2.0 |

## WebUI Dependencies

| Package | License |
|---|---|
| @internationalized/date | Apache-2.0 |
| @lucide/svelte | ISC |
| @novnc/novnc | MPL-2.0 |
| @sveltejs/adapter-static | MIT |
| @sveltejs/kit | MIT |
| @sveltejs/vite-plugin-svelte | MIT |
| @tailwindcss/vite | MIT |
| @xterm/addon-fit | MIT |
| @xterm/addon-web-links | MIT |
| @xterm/xterm | MIT |
| bits-ui | MIT |
| clsx | MIT |
| layerchart | MIT |
| lightningcss | MPL-2.0 |
| simple-icons | CC0-1.0 |
| svelte | MIT |
| svelte-check | MIT |
| tailwind-merge | MIT |
| tailwind-variants | MIT |
| tailwindcss | MIT |
| tw-animate-css | MIT |
| typescript | Apache-2.0 |
| vite | MIT |

## System Components (NixOS)

| Package | License |
|---|---|
| avahi | LGPL-2.1-or-later |
| bcachefs-tools | GPL-2.0 |
| btop | Apache-2.0 |
| caddy | Apache-2.0 |
| croc | MIT |
| diskwatch | MIT |
| docker | Apache-2.0 |
| ethtool | GPL-2.0 |
| fwupd | LGPL-2.1-or-later |
| hdparm | BSD-2-Clause |
| htop | GPL-2.0 |
| iperf3 | BSD-3-Clause |
| iotop-c | GPL-2.0 |
| iproute2 | GPL-2.0 |
| jq | MIT |
| lm-sensors | GPL-2.0 |
| lsof | Zlib |
| nfs-utils | GPL-2.0 |
| netwatch | MIT; bundled FoxIO JA4 mapping data is BSD-3-Clause |
| nvme-cli | GPL-2.0 |
| openssh | BSD-2-Clause |
| OVMF | BSD-2-Clause |
| parted | GPL-3.0 |
| pciutils | GPL-2.0 |
| qemu | GPL-2.0 |
| rsync | GPL-3.0 |
| samba | GPL-3.0 |
| smartmontools | GPL-2.0 |
| syswatch | MIT |
| targetcli-fb | Apache-2.0 |
| tcpdump | BSD-3-Clause |
| util-linux | GPL-2.0 |

## NetWatch JA4 Data Notice

NetWatch includes JA4 TLS client fingerprint mappings from FoxIO's JA4 mapping
database. The bundled data is distributed under the BSD-3-Clause license:

Copyright (c) 2026 FoxIO. All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice,
   this list of conditions and the following disclaimer.
2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.
3. Neither the name of FoxIO nor the names of its contributors may be used to
   endorse or promote products derived from this software without specific
   prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

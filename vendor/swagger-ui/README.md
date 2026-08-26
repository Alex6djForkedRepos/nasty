# Vendored Swagger UI

Vendored assets from the official
[swagger-ui-dist](https://www.npmjs.com/package/swagger-ui-dist) v5.32.14 npm
package:

- Tarball: `https://registry.npmjs.org/swagger-ui-dist/-/swagger-ui-dist-5.32.14.tgz`
- Integrity: `sha512-nOA2pSQhcmODMUQZpJHYKNuwniDUqcOWGNaSCOoZv12FdOSJ9JxV95HtyRGNMqEBj6h6lCNTy20TgZDYTSuUIg==`

Served by the engine at `/api/docs` (via `engine/nasty-engine/src/swagger_ui.rs`)
loading the spec from `/api/openapi.json`. Embedded into the engine binary at
compile time with the `include_dir` macro so the docs page works without any
runtime file dependency — including on air-gapped boxes.

## Files

- `swagger-ui.css` — Stylesheet
- `swagger-ui-bundle.js` — Bundled UI + all standard plugins
- `LICENSE` — Apache 2.0
- `NOTICE` — Upstream attribution notice
- `swagger-ui-bundle.js.LICENSE.txt` — Bundled dependency license notices

## Updating

```sh
ver=<new-version>
tmp=$(mktemp -d)
npm pack "swagger-ui-dist@${ver}" --pack-destination "$tmp"
tar -xzf "$tmp/swagger-ui-dist-${ver}.tgz" -C "$tmp"
for file in swagger-ui.css swagger-ui-bundle.js \
  swagger-ui-bundle.js.LICENSE.txt LICENSE NOTICE; do
  install -m 0644 "$tmp/package/$file" "vendor/swagger-ui/$file"
done
```

Update the version, tarball, and registry integrity in this README, the comment
at the top of `engine/nasty-engine/src/swagger_ui.rs`, and other current-version
references.

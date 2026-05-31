# Vendored Swagger UI

Vendored assets from [swagger-ui-dist](https://www.npmjs.com/package/swagger-ui-dist) v5.17.14.

Served by the engine at `/api/docs` (via `engine/nasty-engine/src/swagger_ui.rs`)
loading the spec from `/api/openapi.json`. Embedded into the engine binary at
compile time with the `include_dir` macro so the docs page works without any
runtime file dependency — including on air-gapped boxes.

## Files

- `swagger-ui.css` — Stylesheet
- `swagger-ui-bundle.js` — Bundled UI + all standard plugins
- `LICENSE` — Apache 2.0

## Updating

```sh
ver=<new-version>
cd vendor/swagger-ui
curl -sLO "https://cdn.jsdelivr.net/npm/swagger-ui-dist@${ver}/swagger-ui.css"
curl -sLO "https://cdn.jsdelivr.net/npm/swagger-ui-dist@${ver}/swagger-ui-bundle.js"
curl -sLO "https://cdn.jsdelivr.net/npm/swagger-ui-dist@${ver}/LICENSE"
```

Update the version number in this README and in the comment at the top of
`engine/nasty-engine/src/swagger_ui.rs`.

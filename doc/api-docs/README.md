# API Docs

## What is available

The backend exposes OpenAPI documentation and an interactive Swagger UI directly from the Axum application.

Available endpoints:

- `GET /openapi.json` returns the OpenAPI document in JSON format.
- `GET /docs` opens the interactive Swagger UI.

## Authentication

The docs endpoints use the same Basic Auth middleware as the rest of the API.

Example:

```powershell
curl -u admin:admin http://127.0.0.1:8080/openapi.json
```

Open in browser:

- `http://127.0.0.1:8080/docs`

## Covered routes

The generated documentation includes:

- system endpoints: `health`, `metrics`
- jobs API
- nodes API
- data API

## Notes

- The docs describe the current Axum HTTP interface.
- `metrics` is documented as `text/plain`.
- Swagger UI is served by the backend itself, so no separate docs container is required.

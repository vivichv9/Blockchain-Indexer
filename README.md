# Bitcoin Blockchain Indexer

Bitcoin Blockchain Indexer is a modular stateless monolith for indexing Bitcoin blockchain data into PostgreSQL and exposing it through a REST API, Swagger UI, admin panel, and CLI.

The backend is written in Rust with Axum, data is stored in PostgreSQL, the admin panel is built with React/Vite, and the whole stack can be started with Docker Compose.

## Features

- Bitcoin JSON-RPC integration over HTTP/HTTPS with Basic Auth
- PostgreSQL-backed canonical blocks, transactions, UTXO, balances, mempool, jobs, and node health
- REST API for data access and runtime operations
- Interactive API docs at `/docs` and OpenAPI JSON at `/openapi.json`
- Admin panel for jobs and node health
- Runtime creation of jobs and monitored nodes without backend restart
- Python CLI for jobs, nodes, and data API

## Architecture

- Architectural style: modular stateless monolith
- Backend: Rust + Axum + SQLx
- Database: PostgreSQL 16
- Admin UI: React + Vite + nginx
- Deployment mode in repository: Docker Compose

## Requirements

To run the project you need:

- Docker Desktop or Docker Engine with `docker compose`
- Access to a Bitcoin JSON-RPC endpoint
- Credentials for:
  - backend Basic Auth
  - Bitcoin RPC Basic Auth

The repository does not start Bitcoin Core for you. You need an existing Bitcoin JSON-RPC endpoint, for example a dedicated Bitcoin Core node behind reverse proxy or a remote RPC service you control.

## Quick Start

### 1. Clone the repository

```powershell
git clone <YOUR_REPOSITORY_URL>
cd Blockchain-Indexer
```

### 2. Create `.env`

```powershell
Copy-Item .env.template .env
```

Fill in the required secrets in `.env`:

```env
INDEXER_API_USERNAME=admin
INDEXER_API_PASSWORD=change-me-api-password
BITCOIN_RPC_PASSWORD=change-me-rpc-password
DATABASE_URL=postgres://indexer:indexer@postgres:5432/indexer
```

### 3. Review `config/indexer.yaml`

The main application config lives in [config/indexer.yaml](config/indexer.yaml).

At minimum, review these fields:

- `server.bind_host`
- `server.bind_port`
- `server.auth.basic.username`
- `rpc.node_id`
- `rpc.url`
- `rpc.auth.basic.username`
- `rpc.insecure_skip_verify`
- `rpc.mtls.enabled`
- `indexer.network`
- `indexer.reorg_depth`
- `jobs`

Important notes:

- API password is loaded from `INDEXER_API_PASSWORD`, not from YAML.
- RPC password is loaded from `BITCOIN_RPC_PASSWORD`, not from YAML.
- For a self-signed RPC TLS certificate, set `rpc.insecure_skip_verify: true`.
- For `address_list` jobs, `addresses` must not be empty.

### 4. Optional: provide certificates

`docker-compose.yml` mounts `./certs` into the backend container as `/app/certs`.

If you enable RPC mTLS in `config/indexer.yaml`, provide:

- `certs/mtls/ca.crt`
- `certs/mtls/client.crt`
- `certs/mtls/client.key`

If you also configure server TLS paths, provide:

- `certs/server.crt`
- `certs/server.key`

Note: the backend validates configured file paths at startup. At the current stage, the config supports TLS settings, but the main Axum application is still served as HTTP on port `8080`.

### 5. Start the stack

```powershell
docker compose up --build
```

Run in background:

```powershell
docker compose up --build -d
```

Stop services:

```powershell
docker compose down
```

Stop services and remove PostgreSQL data:

```powershell
docker compose down -v
```

## What Starts in Docker Compose

The repository starts three services:

- `postgres`
- `backend`
- `admin-panel`

During backend startup the application:

- loads and validates `config/indexer.yaml`
- checks required env vars and certificate paths
- connects to PostgreSQL
- applies SQL migrations from `migrations/`
- synchronizes jobs from YAML into the database
- synchronizes the primary RPC node into the runtime node registry
- starts the HTTP API
- starts background runners for jobs, mempool, and node health

## Service Endpoints

After successful startup:

- Backend API: `http://127.0.0.1:8080`
- Swagger UI: `http://127.0.0.1:8080/docs`
- OpenAPI JSON: `http://127.0.0.1:8080/openapi.json`
- Admin panel: `http://127.0.0.1:4173`

## Verify the Installation

Health check:

```powershell
curl http://127.0.0.1:8080/health
```

Jobs API:

```powershell
curl -u admin:change-me-api-password http://127.0.0.1:8080/v1/jobs
```

Nodes API:

```powershell
curl -u admin:change-me-api-password http://127.0.0.1:8080/v1/nodes
```

Data API:

```powershell
curl -u admin:change-me-api-password "http://127.0.0.1:8080/v1/data/transactions?limit=10"
```

OpenAPI document:

```powershell
curl -u admin:change-me-api-password http://127.0.0.1:8080/openapi.json
```

## Runtime Operations Without Restart

### Create a new indexing job

```powershell
curl -u admin:change-me-api-password ^
  -H "Content-Type: application/json" ^
  -X POST http://127.0.0.1:8080/v1/jobs ^
  -d "{\"job_id\":\"watchlist-runtime\",\"mode\":\"address_list\",\"enabled\":true,\"addresses\":[\"addr1\",\"addr2\"]}"
```

### Add a new monitored node

```powershell
curl -u admin:change-me-api-password ^
  -H "Content-Type: application/json" ^
  -X POST http://127.0.0.1:8080/v1/nodes ^
  -d "{\"node_id\":\"btc-testnet-2\",\"url\":\"https://rpc.example.com\",\"username\":\"rpcuser\",\"password\":\"secret\",\"insecure_skip_verify\":false,\"enabled\":true}"
```

## Admin Panel

The admin panel is included in Docker Compose and is available immediately after startup at `http://127.0.0.1:4173`.

Currently it provides:

- `Jobs` page
- `Node Health` page
- create job form
- create node form
- job state control actions

More details: [doc/admin-panel/README.md](doc/admin-panel/README.md)

## CLI

The repository also includes a Python CLI in [cli/indexer_cli.py](cli/indexer_cli.py).

Examples:

```powershell
python cli/indexer_cli.py --base-url http://127.0.0.1:8080 --username admin --password change-me-api-password jobs list
python cli/indexer_cli.py --base-url http://127.0.0.1:8080 --username admin --password change-me-api-password nodes list
python cli/indexer_cli.py --base-url http://127.0.0.1:8080 --username admin --password change-me-api-password data txs --limit 10
```

## Repository Structure

- `src/` - Rust backend
- `admin-panel/` - React/Vite admin panel
- `cli/` - Python CLI
- `config/` - application configuration
- `migrations/` - SQL migrations
- `doc/` - module-level documentation
- `scripts/` - helper scripts

## Useful Documentation

- Config and auth: [doc/config-and-auth/README.md](doc/config-and-auth/README.md)
- API docs: [doc/api-docs/README.md](doc/api-docs/README.md)
- Database schema: [doc/database-schema/README.md](doc/database-schema/README.md)
- Indexer: [doc/indexer/README.md](doc/indexer/README.md)
- Mempool: [doc/mempool/README.md](doc/mempool/README.md)
- Reorg handling: [doc/reorg/README.md](doc/reorg/README.md)
- Jobs: [doc/jobs/README.md](doc/jobs/README.md)
- Nodes: [doc/nodes/README.md](doc/nodes/README.md)
- Data API: [doc/data-api/README.md](doc/data-api/README.md)
- Testing: [doc/testing/README.md](doc/testing/README.md)
- CLI: [doc/cli/README.md](doc/cli/README.md)
- Acceptance checklist: [doc/acceptance/README.md](doc/acceptance/README.md)

## Current Limitations

- Docker Compose starts PostgreSQL, backend, and admin panel, but not Bitcoin Core.
- A working Bitcoin JSON-RPC endpoint must be provided separately.
- The backend currently serves HTTP on port `8080`.
- Production hardening, full HTTPS termination, and real end-to-end regtest verification still need separate confirmation.


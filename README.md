# Bitcoin Blockchain Indexer

Bitcoin Blockchain Indexer — это индексер блокчейна Bitcoin в формате модульного stateless-монолита. Проект сохраняет данные в PostgreSQL и отдает их через REST API, Swagger UI, admin panel и CLI.

Backend написан на Rust с Axum, данные хранятся в PostgreSQL, admin panel собрана на React/Vite, а весь стек можно запустить через Docker Compose.

## Возможности

- интеграция с Bitcoin JSON-RPC по HTTP/HTTPS с Basic Auth
- хранение canonical blocks, transactions, UTXO, balances, mempool, jobs и node health в PostgreSQL
- REST API для выдачи данных и runtime-операций
- интерактивная документация API на `/docs` и OpenAPI JSON на `/openapi.json`
- admin panel для управления jobs и мониторинга узлов
- создание jobs и monitored nodes без перезапуска backend
- Python CLI для работы с jobs, nodes и data API

## Архитектура

- архитектурный стиль: модульный stateless-монолит
- backend: Rust + Axum + SQLx
- база данных: PostgreSQL 16
- admin UI: React + Vite + nginx
- способ развёртывания в репозитории: Docker Compose

## Что нужно для запуска

Для запуска проекта нужны:

- Docker Desktop или Docker Engine с `docker compose`
- доступный Bitcoin JSON-RPC endpoint
- учётные данные для:
  - Basic Auth backend API
  - Basic Auth Bitcoin RPC

Репозиторий не поднимает Bitcoin Core автоматически. Нужен уже существующий Bitcoin JSON-RPC endpoint, например собственный Bitcoin Core за reverse proxy или отдельный удалённый RPC-сервис.

## Быстрый старт

### 1. Клонируй репозиторий

```powershell
git clone https://github.com/vivichv9/Blockchain-Indexer.git
cd Blockchain-Indexer
```

### 2. Создай `.env`

```powershell
cp .env.template .env
```

Заполни обязательные секреты в `.env`:

```env
INDEXER_API_USERNAME=admin
INDEXER_API_PASSWORD=change-me-api-password
BITCOIN_RPC_PASSWORD=change-me-rpc-password
DATABASE_URL=postgres://indexer:indexer@postgres:5432/indexer
```

### 3. Проверь `config/indexer.yaml`

Основной конфиг приложения находится в [config/indexer.yaml](config/indexer.yaml).

Перед первым запуском обязательно проверь:

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

Важно:

- пароль API берётся из `INDEXER_API_PASSWORD`, а не из YAML
- пароль RPC берётся из `BITCOIN_RPC_PASSWORD`, а не из YAML
- если у RPC self-signed TLS сертификат, включи `rpc.insecure_skip_verify: true`
- для jobs с `mode: address_list` список `addresses` не должен быть пустым

### 4. При необходимости подготовь сертификаты

`docker-compose.yml` монтирует каталог `./certs` в контейнер backend как `/app/certs`.

Если в `config/indexer.yaml` включён RPC mTLS, должны существовать:

- `certs/mtls/ca.crt`
- `certs/mtls/client.crt`
- `certs/mtls/client.key`

Если дополнительно настроены пути для server TLS, должны существовать:

- `certs/server.crt`
- `certs/server.key`

Примечание: backend валидирует наличие указанных файлов при старте. На текущем этапе конфиг поддерживает TLS-поля, но основное Axum-приложение всё ещё публикуется как HTTP-сервис на порту `8080`.

### 5. Подними стек

```powershell
docker compose up --build
```

Запуск в фоне:

```powershell
docker compose up --build -d
```

Остановить сервисы:

```powershell
docker compose down
```

Остановить сервисы и удалить данные PostgreSQL:

```powershell
docker compose down -v
```

## Что поднимается в Docker Compose

Репозиторий поднимает три сервиса:

- `postgres`
- `backend`
- `admin-panel`

Во время старта backend приложение:

- загружает и валидирует `config/indexer.yaml`
- проверяет обязательные env-переменные и пути к сертификатам
- подключается к PostgreSQL
- применяет SQL-миграции из `migrations/`
- синхронизирует jobs из YAML в базу данных
- синхронизирует основной RPC-узел в runtime-реестр узлов
- запускает HTTP API
- запускает фоновые runners для jobs, mempool и node health

## Адреса сервисов

После успешного запуска доступны:

- backend API: `http://127.0.0.1:8080`
- Swagger UI: `http://127.0.0.1:8080/docs`
- OpenAPI JSON: `http://127.0.0.1:8080/openapi.json`
- admin panel: `http://127.0.0.1:4173`

## Как проверить запуск

Проверка health:

```powershell
curl http://127.0.0.1:8080/health
```

Проверка jobs API:

```powershell
curl -u admin:change-me-api-password http://127.0.0.1:8080/v1/jobs
```

Проверка nodes API:

```powershell
curl -u admin:change-me-api-password http://127.0.0.1:8080/v1/nodes
```

Проверка data API:

```powershell
curl -u admin:change-me-api-password "http://127.0.0.1:8080/v1/data/transactions?limit=10"
```

Проверка OpenAPI:

```powershell
curl -u admin:change-me-api-password http://127.0.0.1:8080/openapi.json
```

## Runtime-операции без перезапуска

### Создание новой job

```powershell
curl -u admin:change-me-api-password ^
  -H "Content-Type: application/json" ^
  -X POST http://127.0.0.1:8080/v1/jobs ^
  -d "{\"job_id\":\"watchlist-runtime\",\"mode\":\"address_list\",\"enabled\":true,\"addresses\":[\"addr1\",\"addr2\"]}"
```

### Добавление нового узла

```powershell
curl -u admin:change-me-api-password ^
  -H "Content-Type: application/json" ^
  -X POST http://127.0.0.1:8080/v1/nodes ^
  -d "{\"node_id\":\"btc-testnet-2\",\"url\":\"https://rpc.example.com\",\"username\":\"rpcuser\",\"password\":\"secret\",\"insecure_skip_verify\":false,\"enabled\":true}"
```

## Admin Panel

Admin panel входит в `docker compose` и после запуска доступна по адресу `http://127.0.0.1:4173`.

Сейчас в ней доступны:

- страница `Jobs`
- страница `Node Health`
- форма создания job
- форма добавления узла
- действия управления состоянием job

Подробнее: [doc/admin-panel/README.md](doc/admin-panel/README.md)

## CLI

В репозитории также есть Python CLI: [cli/indexer_cli.py](cli/indexer_cli.py).

Примеры:

```powershell
python cli/indexer_cli.py --base-url http://127.0.0.1:8080 --username admin --password change-me-api-password jobs list
python cli/indexer_cli.py --base-url http://127.0.0.1:8080 --username admin --password change-me-api-password nodes list
python cli/indexer_cli.py --base-url http://127.0.0.1:8080 --username admin --password change-me-api-password data txs --limit 10
```

## Структура репозитория

- `src/` — Rust backend
- `admin-panel/` — React/Vite admin panel
- `cli/` — Python CLI
- `config/` — конфигурация приложения
- `migrations/` — SQL-миграции
- `doc/` — модульная документация
- `scripts/` — вспомогательные скрипты

## Полезная документация

- конфиг и авторизация: [doc/config-and-auth/README.md](doc/config-and-auth/README.md)
- документация API: [doc/api-docs/README.md](doc/api-docs/README.md)
- схема БД: [doc/database-schema/README.md](doc/database-schema/README.md)
- индексатор: [doc/indexer/README.md](doc/indexer/README.md)
- mempool: [doc/mempool/README.md](doc/mempool/README.md)
- обработка reorg: [doc/reorg/README.md](doc/reorg/README.md)
- jobs: [doc/jobs/README.md](doc/jobs/README.md)
- nodes: [doc/nodes/README.md](doc/nodes/README.md)
- data API: [doc/data-api/README.md](doc/data-api/README.md)
- тестирование: [doc/testing/README.md](doc/testing/README.md)
- CLI: [doc/cli/README.md](doc/cli/README.md)
- acceptance checklist: [doc/acceptance/README.md](doc/acceptance/README.md)

## Текущие ограничения

- `docker compose` поднимает PostgreSQL, backend и admin panel, но не поднимает Bitcoin Core
- рабочий Bitcoin JSON-RPC endpoint нужно предоставить отдельно
- backend сейчас публикуется как HTTP-сервис на порту `8080`
- production hardening, полноценный HTTPS-контур и полная end-to-end проверка с regtest требуют отдельного подтверждения

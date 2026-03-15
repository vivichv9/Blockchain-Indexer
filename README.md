# Bitcoin Blockchain Indexer

Индексер блокчейна Bitcoin в формате модульного stateless-монолита. Backend написан на Rust, хранение идёт в PostgreSQL, управление и выдача данных доступны через REST API, admin panel и CLI.

## Что нужно для развёртывания
- Docker Desktop или Docker Engine с поддержкой `docker compose`
- доступный PostgreSQL через `docker-compose` из репозитория
- YAML-конфиг индексера
- секреты для Basic Auth API и RPC
- TLS/mTLS сертификаты в файловой системе, если они включены в конфиге

## Структура развёртывания
- `docker-compose.yml` поднимает:
  - `postgres`
  - `backend`
  - `admin-panel`
- backend читает:
  - `.env`
  - `config/indexer.yaml`
  - `certs/`
  - `migrations/`

## 1. Подготовка окружения
Скопируй шаблон env и заполни секреты:

```powershell
Copy-Item .env.template .env
```

Минимально нужно задать в `.env`:

```env
INDEXER_API_PASSWORD=change-me-api-password
BITCOIN_RPC_PASSWORD=change-me-rpc-password
DATABASE_URL=postgres://indexer:indexer@postgres:5432/indexer
```

По умолчанию backend внутри compose использует:
- `DATABASE_URL=postgres://indexer:indexer@postgres:5432/indexer`
- `INDEXER_CONFIG_PATH=/app/config/indexer.yaml`
- `MIGRATIONS_PATH=/app/migrations`

Для admin panel compose использует:
- `INDEXER_API_USERNAME`
- `INDEXER_API_PASSWORD`

## 2. Подготовка конфига
Базовый конфиг лежит в [indexer.yaml](/D:/Users/Кирилл/Documents/Blockchain-Indexer/config/indexer.yaml).

Проверь и при необходимости измени:
- `server.bind_host`
- `server.bind_port`
- `server.auth.basic.username`
- `rpc.url`
- `rpc.auth.basic.username`
- `rpc.mtls.enabled`
- `indexer.network`
- `indexer.reorg_depth`
- `jobs`

Важно:
- пароль API берётся не из YAML, а из env-переменной `INDEXER_API_PASSWORD`
- пароль RPC берётся не из YAML, а из env-переменной `BITCOIN_RPC_PASSWORD`
- если `rpc.mtls.enabled: true`, все пути к сертификатам должны существовать и читаться при старте
- для `address_list` job список `addresses` не может быть пустым

## 3. Подготовка сертификатов
Текущий `docker-compose.yml` монтирует каталог `./certs` в контейнер как `/app/certs`.

Если в `config/indexer.yaml` включён mTLS, должны существовать файлы:
- `certs/mtls/ca.crt`
- `certs/mtls/client.crt`
- `certs/mtls/client.key`

Если в `server.tls` указаны пути:
- `certs/server.crt`
- `certs/server.key`

Примечание:
- конфиг уже валидирует наличие этих файлов на старте
- текущая реализация backend читает TLS-пути из конфига, но сам HTTP-сервер сейчас поднимается как обычный Axum listener без подтверждённого HTTPS-контура

## 4. Запуск индексера
Собери и подними сервисы:

```powershell
docker compose up --build
```

Для запуска в фоне:

```powershell
docker compose up --build -d
```

Остановить сервисы:

```powershell
docker compose down
```

Остановить сервисы и удалить volume PostgreSQL:

```powershell
docker compose down -v
```

## 5. Что происходит при старте backend
При запуске backend:
- загружает YAML-конфиг
- валидирует пути к сертификатам и обязательные env
- подключается к PostgreSQL
- применяет SQL-миграции из `migrations/`
- синхронизирует jobs из YAML в БД
- синхронизирует основной RPC-узел в runtime-реестр `nodes_registry`
- поднимает API
- запускает background runners:
  - jobs runner
  - mempool runner
  - nodes health runner

## 6. Проверка после запуска
Если compose поднят локально, backend опубликован на порту `8080`.
Admin panel опубликована на порту `4173`.

Проверка health:

```powershell
curl http://127.0.0.1:8080/health
```

Проверка jobs API с Basic Auth:

```powershell
curl -u admin:change-me-api-password http://127.0.0.1:8080/v1/jobs
```

Создание job без перезапуска:

```powershell
curl -u admin:change-me-api-password ^
  -H "Content-Type: application/json" ^
  -X POST http://127.0.0.1:8080/v1/jobs ^
  -d "{\"job_id\":\"watchlist-runtime\",\"mode\":\"address_list\",\"enabled\":true,\"addresses\":[\"addr1\",\"addr2\"]}"
```

Проверка nodes API:

```powershell
curl -u admin:change-me-api-password http://127.0.0.1:8080/v1/nodes
```

Добавление узла без перезапуска:

```powershell
curl -u admin:change-me-api-password ^
  -H "Content-Type: application/json" ^
  -X POST http://127.0.0.1:8080/v1/nodes ^
  -d "{\"node_id\":\"btc-testnet-2\",\"url\":\"https://rpc.example.com\",\"username\":\"user\",\"password\":\"secret\",\"insecure_skip_verify\":true,\"enabled\":true}"
```

Проверка data API:

```powershell
curl -u admin:change-me-api-password "http://127.0.0.1:8080/v1/data/transactions?limit=10"
```

Проверка OpenAPI:

```powershell
curl -u admin:change-me-api-password http://127.0.0.1:8080/openapi.json
```

Открыть Swagger UI:

```text
http://127.0.0.1:8080/docs
```

Открыть UI:

```text
http://127.0.0.1:4173
```

## 7. Admin Panel и CLI
Admin panel находится в `admin-panel/` и работает только через REST API backend.
В `docker compose` она поднимается автоматически и отдаётся через `nginx`.

CLI находится в [indexer_cli.py](/D:/Users/Кирилл/Documents/Blockchain-Indexer/cli/indexer_cli.py).

Примеры CLI:

```powershell
python cli/indexer_cli.py --base-url http://127.0.0.1:8080 --username admin --password change-me-api-password jobs list
python cli/indexer_cli.py --base-url http://127.0.0.1:8080 --username admin --password change-me-api-password nodes list
python cli/indexer_cli.py --base-url http://127.0.0.1:8080 --username admin --password change-me-api-password data txs --limit 10
```

## 8. Полезные документы
- Конфиг и авторизация: [README.md](/D:/Users/Кирилл/Documents/Blockchain-Indexer/doc/config-and-auth/README.md)
- Схема БД: [README.md](/D:/Users/Кирилл/Documents/Blockchain-Indexer/doc/database-schema/README.md)
- Индексатор: [README.md](/D:/Users/Кирилл/Documents/Blockchain-Indexer/doc/indexer/README.md)
- Mempool: [README.md](/D:/Users/Кирилл/Documents/Blockchain-Indexer/doc/mempool/README.md)
- Reorg: [README.md](/D:/Users/Кирилл/Documents/Blockchain-Indexer/doc/reorg/README.md)
- Тестирование: [README.md](/D:/Users/Кирилл/Documents/Blockchain-Indexer/doc/testing/README.md)
- CLI: [README.md](/D:/Users/Кирилл/Documents/Blockchain-Indexer/doc/cli/README.md)
- Acceptance checklist: [README.md](/D:/Users/Кирилл/Documents/Blockchain-Indexer/doc/acceptance/README.md)

## 9. Текущие ограничения
- `docker-compose.yml` поднимает PostgreSQL, backend и admin-panel, но не поднимает рядом Bitcoin Core
- backend сейчас опубликован как HTTP-сервис на `:8080`
- для полного production-like развёртывания ещё нужно отдельно подтвердить:
  - рабочий HTTPS-контур backend
  - end-to-end интеграцию с реальным Bitcoin Core/regtest
  - фактическое тестовое покрытие `>= 80%`

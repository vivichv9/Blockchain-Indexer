# CLI

## Что реализовано
- Добавлен Python CLI без внешних зависимостей в `cli/indexer_cli.py`.
- CLI поддерживает конфигурацию backend URL и Basic Auth через аргументы командной строки:
  - `--base-url`
  - `--username`
  - `--password`
- Те же параметры можно передавать через env:
  - `INDEXER_API_BASE_URL`
  - `INDEXER_API_USERNAME`
  - `INDEXER_API_PASSWORD`
- Реализованы команды для `jobs`:
  - `list`
  - `get <job_id>`
  - `start <job_id>`
  - `stop <job_id>`
  - `pause <job_id>`
  - `resume <job_id>`
  - `retry <job_id>`
- Реализованы команды для `nodes`:
  - `list`
  - `health <node_id>`
- Реализованы demo-команды для `data API`:
  - `balance <address>`
  - `utxos <address>`
  - `txs`
  - `mempool`
  - `blocks`

## Где находится
- CLI: `cli/indexer_cli.py`.

## Примеры запуска
- `python cli/indexer_cli.py --base-url http://127.0.0.1:8080 --username admin --password secret jobs list`
- `python cli/indexer_cli.py jobs get full-sync`
- `python cli/indexer_cli.py jobs start full-sync`
- `python cli/indexer_cli.py nodes list`
- `python cli/indexer_cli.py nodes health btc-mainnet-1`
- `python cli/indexer_cli.py data balance bc1... --to-height 100`
- `python cli/indexer_cli.py data utxos bc1...`
- `python cli/indexer_cli.py data txs --address bc1... --limit 20`
- `python cli/indexer_cli.py data mempool --address bc1...`
- `python cli/indexer_cli.py data blocks --has-txid <txid>`

## Поведение
- Все ответы выводятся как форматированный JSON.
- Ошибки API отображаются в stderr с кодом HTTP, `code`, `message` и `details`, если они пришли от backend.
- Если параметры авторизации не переданы, CLI завершится с диагностической ошибкой.

## Ограничения текущего этапа
- CLI пока не содержит отдельного config-файла и использует только argv/env.
- Локальный запуск нужно проверять в окружении, где доступен Python.

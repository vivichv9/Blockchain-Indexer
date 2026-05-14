# Бенчмарки производительности

Функциональная единица содержит воспроизводимый сценарий сравнения HTTP API
Bitcoin Blockchain Indexer и JSON-RPC интерфейса Bitcoin Core.

## Назначение

Скрипт `scripts/benchmark_rpc_vs_indexer.py` измеряет задержку и пропускную
способность повторяющихся запросов к двум источникам данных:

- Bitcoin Blockchain Indexer: HTTP endpoints `/v1/data/...`;
- Bitcoin Core: JSON-RPC методы `getblockchaininfo`, `getblock`,
  `getrawtransaction`, `scantxoutset`.

Итоговый отчет содержит `avg`, `p50`, `p95`, `p99`, `min`, `max`,
`success_rate` и расчет ускорения по медианной задержке для сценариев, где
заданы оба источника.

## Конфигурация

За основу используется файл `scripts/benchmark_rpc_vs_indexer.example.json`.
Перед запуском его нужно скопировать в локальный конфиг и заменить значения:

- `indexer.base_url` — адрес запущенного API индексера;
- `bitcoin_core.rpc_url` — URL JSON-RPC Bitcoin Core;
- `BITCOIN_RPC_PASSWORD` — пароль RPC из переменной окружения;
- `block_hash`, `txid`, `address` — реальные идентификаторы из тестируемой
  цепочки.

Сценарии `transaction_by_txid` и `address_utxo_lookup` отключены по умолчанию,
потому что требуют подготовленных входных данных. Для произвольных исторических
транзакций Bitcoin Core должен быть запущен с `txindex=1`.

## Методика

1. Запустить Bitcoin Core, дождаться полной синхронизации и убедиться, что
   JSON-RPC доступен.
2. Запустить Bitcoin Blockchain Indexer на той же машине или в той же локальной
   сети.
3. Заполнить локальный benchmark-конфиг реальными значениями.
4. Выполнить прогревочные запросы, которые не попадают в итоговую статистику.
5. Выполнить одинаковое количество измеряемых запросов к каждому источнику.
6. Сравнивать `p50` и `p95`, а не только среднее значение, чтобы учитывать
   хвосты задержек.

Пример запуска:

```powershell
$env:BITCOIN_RPC_PASSWORD = "rpc-password"
python scripts/benchmark_rpc_vs_indexer.py --config scripts/benchmark_rpc_vs_indexer.local.json
```

Отчеты создаются в `reports/benchmarks` в форматах JSON и CSV.

## Ограничения сравнения

Bitcoin Core RPC не является специализированным адресным индексом. Поэтому
запросы по адресам в индексере и `scantxoutset` в Bitcoin Core необходимо
описывать как сравнение индексированного поиска с базовой возможностью
сканирования UTXO-набора, а не как полностью эквивалентные операции.

В дипломе и презентации следует использовать только фактически измеренные
значения или явно помеченные расчетные оценки. Если результаты получены на
локальной машине, нужно указывать конфигурацию стенда, версию Bitcoin Core,
количество итераций и параметры запуска узла.

## Справочные источники

Официальная документация Bitcoin Core описывает семантику RPC-методов, но не
публикует универсальные показатели задержки для произвольного аппаратного
стенда. Поэтому основой для дипломной таблицы должны быть собственные замеры,
полученные данным скриптом.

- `getblockchaininfo`: https://bitcoincore.org/en/doc/29.0.0/rpc/blockchain/getblockchaininfo/
- `getblock`: https://bitcoincore.org/en/doc/29.0.0/rpc/blockchain/getblock/
- `getrawtransaction`: https://bitcoincore.org/en/doc/29.0.0/rpc/rawtransactions/getrawtransaction/
- `scantxoutset`: https://bitcoincore.org/en/doc/29.0.0/rpc/blockchain/scantxoutset/
- Bitcoin Core benchmarking framework: https://casey.github.io/bitcoin/doc/benchmarking.html

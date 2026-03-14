# Mempool

## Что реализовано
- Добавлен отдельный `MempoolRunner`, который периодически опрашивает Bitcoin RPC по `indexer.poll.mempool_interval_ms`.
- При синхронизации runner:
  - получает текущий список `txid` через `getrawmempool`,
  - для новых `txid` загружает decoded-транзакцию через `getrawtransaction`,
  - сохраняет транзакцию в `transactions` со статусом `mempool`,
  - сохраняет `vin/vout` в `tx_inputs` и `tx_outputs` для последующей фильтрации по адресу,
  - помечает исчезнувшие из mempool неподтвержденные транзакции как `dropped`.
- Подтвержденные агрегаты (`utxos_current`, `address_balance_current`, `address_balance_history`) не смешиваются с mempool и продолжают отражать только canonical confirmed-цепочку.
- Добавлен query-helper для выборки mempool-транзакций по адресу на основе `inputs/outputs`.

## Где находится
- Runner и синхронизация mempool: `src/modules/mempool/mod.rs`.
- RPC-методы `getrawmempool` и verbose `getrawtransaction`: `src/modules/rpc/mod.rs`.
- Инициализация и запуск runner: `src/app.rs`.

## Ограничения этапа
- Mempool-данные пока только сохраняются и помечаются как `dropped`; REST endpoint для выдачи mempool еще не добавлен.
- Для mempool не пересчитываются current UTXO и confirmed-балансы, чтобы не смешивать неподтвержденное состояние с canonical-данными.
- Если транзакция исчезла из mempool между получением списка и запросом decoded-версии, runner только логирует предупреждение и продолжает синхронизацию.

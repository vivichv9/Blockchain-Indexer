# Data API

## Что реализовано
- Добавлены data-endpoint'ы:
  - `GET /v1/data/addresses/{address}/balance`
  - `GET /v1/data/addresses/{address}/utxos`
  - `GET /v1/data/transactions`
  - `GET /v1/data/transactions/mempool`
  - `GET /v1/data/blocks`
- Для списковых endpoint'ов поддержана пагинация через `offset` и `limit` с валидацией:
  - `offset >= 0`
  - `limit` в диапазоне `1..1000`
- Для запросов с фильтром по адресу добавлена проверка, что адрес входит в область индексации:
  - если есть активный job `all_addresses`, адрес считается проиндексированным,
  - иначе адрес должен присутствовать в `job_addresses` активного job,
  - если адрес не покрыт индексацией, API возвращает `404 ADDRESS_NOT_INDEXED`.
- Балансы и UTXO отдаются только из confirmed/canonical-данных.
- Mempool endpoint отдает только `status=mempool`.
- Исторический balance query с `from_height` / `to_height` и `from_time` / `to_time` корректно работает как для выборки tip-блока, так и для списка блоков.

## Где находится
- HTTP-обработчики и маппинг ошибок: `src/modules/api/mod.rs`.
- Data-запросы к PostgreSQL: `src/modules/data/mod.rs`.
- Синхронизация `job_addresses` из YAML: `src/modules/jobs/mod.rs`.

## Ограничения этапа
- Фильтрация по адресу для `transactions` и `blocks` опирается на `tx_inputs/tx_outputs` и не использует отдельную materialized-address-view.
- Для проиндексированного адреса без подтвержденной истории баланс может возвращаться как `0`.
- Формат выдачи блоков и транзакций минимальный и ориентирован на текущее ТЗ; расширенные DTO можно добавить позже без изменения контрактов фильтрации и пагинации.

# Database Schema

## Что реализовано
- Миграция `migrations/0001_init.sql` теперь содержит минимально обязательную схему PostgreSQL из ТЗ:
  - `blocks`
  - `transactions`
  - `tx_outputs`
  - `tx_inputs`
  - `utxos_current`
  - `address_balance_current`
  - `address_balance_history`
  - `jobs`
  - `job_addresses`
  - `node_health`
- Для ключевых таблиц добавлены индексы и ограничения целостности.
- Для статусных полей добавлены `CHECK`-ограничения допустимых значений.

## Цель этапа
- Зафиксировать контракт хранения данных для последующих модулей `storage`, `indexer`, `jobs`, `api`.
- Обеспечить базовую идемпотентность на уровне БД через `PRIMARY KEY` и `UNIQUE`.

## Ограничения этапа
- Нет yet-логики применения миграций в рантайме backend.
- Нет yet-логики пересчета агрегатов при reorg на уровне приложения.

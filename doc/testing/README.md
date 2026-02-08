# Testing

## Что реализовано
- Добавлены интеграционные тесты для jobs lifecycle API.
- Тесты поднимают PostgreSQL через `testcontainers` и выполняют миграции.

## Где находится
- Интеграционные тесты: `tests/integration_jobs_api.rs`.

## Требования к запуску
- Нужен запущенный Docker (тесты используют `testcontainers`).
- Интеграционные тесты помечены `#[ignore]` и запускаются вручную:
  - `cargo test -- --ignored`
  - или `cargo test --test integration_jobs_api -- --ignored`

## Ограничения этапа
- Тесты пока покрывают только jobs API.
- Для других модулей добавим интеграционные тесты по мере реализации.

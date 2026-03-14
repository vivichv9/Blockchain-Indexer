# RPC

## Что реализовано
- RPC-клиент для Bitcoin Core с поддержкой mTLS (опционально) и Basic Auth.
- Таймауты соединения и запроса берутся из `rpc.timeouts`.
- Базовые RPC методы: `getblockhash`, `getblock`, `getrawtransaction`.

## Где находится
- RPC-клиент: `src/modules/rpc/mod.rs`.

## Как используется
- Клиент создается через `RpcClient::from_config` на основе `rpc` секции YAML.
- Отключение mTLS: `rpc.mtls.enabled: false`.
- Все вызовы проходят через JSON-RPC `call`.

## Ограничения этапа
- Ретраи и троттлинг пока не реализованы.
- Ошибки RPC возвращаются как строка `message`.

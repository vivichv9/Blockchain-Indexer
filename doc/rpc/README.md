# RPC

## Что реализовано
- RPC-клиент для Bitcoin Core с поддержкой mTLS (опционально) и Basic Auth.
- Таймауты соединения и запроса берутся из `rpc.timeouts`.
- Базовые RPC методы: `getblockhash`, `getblock`, `getrawtransaction`.
- HTTP/RPC ошибки логируются с расширенной диагностикой: URL, HTTP status, kind (`connect`/`timeout`/`decode`/...) и цепочка внутренних source-ошибок.
- Для endpoint'ов с self-signed TLS-сертификатом можно явно включить `rpc.insecure_skip_verify: true`, чтобы отключить проверку доверия серверного сертификата.

## Где находится
- RPC-клиент: `src/modules/rpc/mod.rs`.

## Как используется
- Клиент создается через `RpcClient::from_config` на основе `rpc` секции YAML.
- Отключение mTLS: `rpc.mtls.enabled: false`.
- Отключение проверки TLS-сертификата RPC: `rpc.insecure_skip_verify: true`.
- Все вызовы проходят через JSON-RPC `call`.

## Ограничения этапа
- Ретраи и троттлинг пока не реализованы.
- Ошибки RPC возвращаются как строка `message`.
- `rpc.insecure_skip_verify: true` снижает безопасность соединения и должен использоваться только там, где self-signed TLS осознанно принят как компромисс.

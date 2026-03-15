#!/usr/bin/env python3
import argparse
import base64
import json
import os
import sys
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from typing import Any, Dict, Optional


DEFAULT_BASE_URL = "http://127.0.0.1:8080"


class CliError(Exception):
    pass


@dataclass
class CliConfig:
    base_url: str
    username: str
    password: str


class ApiClient:
    def __init__(self, config: CliConfig) -> None:
        self._config = config

    def get(self, path: str, query: Optional[Dict[str, Any]] = None) -> Any:
        return self._request("GET", path, query=query)

    def post(self, path: str) -> Any:
        return self._request("POST", path)

    def _request(
        self,
        method: str,
        path: str,
        query: Optional[Dict[str, Any]] = None,
    ) -> Any:
        base_url = self._config.base_url.rstrip("/")
        url = f"{base_url}{path}"
        if query:
            pairs = [(key, value) for key, value in query.items() if value is not None]
            if pairs:
                url = f"{url}?{urllib.parse.urlencode(pairs)}"

        credentials = f"{self._config.username}:{self._config.password}".encode("utf-8")
        auth_header = base64.b64encode(credentials).decode("ascii")

        request = urllib.request.Request(
            url,
            method=method,
            headers={
                "Authorization": f"Basic {auth_header}",
                "Content-Type": "application/json",
            },
        )

        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                payload = response.read().decode("utf-8")
        except urllib.error.HTTPError as exc:
            body = exc.read().decode("utf-8", errors="replace")
            raise CliError(format_http_error(exc.code, body)) from exc
        except urllib.error.URLError as exc:
            raise CliError(f"request failed: {exc.reason}") from exc

        if not payload:
            return None

        try:
            return json.loads(payload)
        except json.JSONDecodeError as exc:
            raise CliError(f"failed to decode JSON response: {exc}") from exc


def format_http_error(status_code: int, body: str) -> str:
    try:
        payload = json.loads(body)
    except json.JSONDecodeError:
        return f"HTTP {status_code}: {body or 'empty response body'}"

    code = payload.get("code", "HTTP_ERROR")
    message = payload.get("message", "Request failed")
    details = payload.get("details")
    if details:
        return f"HTTP {status_code}: {code}: {message} ({json.dumps(details, ensure_ascii=False)})"
    return f"HTTP {status_code}: {code}: {message}"


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="indexer-cli",
        description="CLI for Bitcoin Blockchain Indexer REST API.",
    )
    parser.add_argument(
        "--base-url",
        default=os.getenv("INDEXER_API_BASE_URL", DEFAULT_BASE_URL),
        help="Base URL of the backend API. Default: env INDEXER_API_BASE_URL or http://127.0.0.1:8080",
    )
    parser.add_argument(
        "--username",
        default=os.getenv("INDEXER_API_USERNAME", ""),
        help="Basic Auth username. Default: env INDEXER_API_USERNAME",
    )
    parser.add_argument(
        "--password",
        default=os.getenv("INDEXER_API_PASSWORD", ""),
        help="Basic Auth password. Default: env INDEXER_API_PASSWORD",
    )

    subparsers = parser.add_subparsers(dest="resource", required=True)

    jobs_parser = subparsers.add_parser("jobs", help="Manage indexing jobs")
    jobs_subparsers = jobs_parser.add_subparsers(dest="action", required=True)
    jobs_subparsers.add_parser("list", help="List jobs")

    jobs_get = jobs_subparsers.add_parser("get", help="Get job details")
    jobs_get.add_argument("job_id", help="Job identifier")

    for action in ("start", "stop", "pause", "resume", "retry"):
        job_action = jobs_subparsers.add_parser(action, help=f"{action.title()} a job")
        job_action.add_argument("job_id", help="Job identifier")

    nodes_parser = subparsers.add_parser("nodes", help="Inspect node health")
    nodes_subparsers = nodes_parser.add_subparsers(dest="action", required=True)
    nodes_subparsers.add_parser("list", help="List known nodes")

    nodes_get = nodes_subparsers.add_parser("health", help="Get detailed node health")
    nodes_get.add_argument("node_id", help="Node identifier")

    data_parser = subparsers.add_parser("data", help="Query indexed blockchain data")
    data_subparsers = data_parser.add_subparsers(dest="action", required=True)

    balance_parser = data_subparsers.add_parser("balance", help="Get address balance")
    balance_parser.add_argument("address", help="Bitcoin address")
    balance_parser.add_argument("--from-time", type=int, default=None)
    balance_parser.add_argument("--to-time", type=int, default=None)
    balance_parser.add_argument("--from-height", type=int, default=None)
    balance_parser.add_argument("--to-height", type=int, default=None)

    balance_history_parser = data_subparsers.add_parser(
        "balance-history",
        help="Get balance history snapshots for an address",
    )
    balance_history_parser.add_argument("address", help="Bitcoin address")
    balance_history_parser.add_argument("--from-time", type=int, default=None)
    balance_history_parser.add_argument("--to-time", type=int, default=None)
    balance_history_parser.add_argument("--from-height", type=int, default=None)
    balance_history_parser.add_argument("--to-height", type=int, default=None)
    balance_history_parser.add_argument("--offset", type=int, default=None)
    balance_history_parser.add_argument("--limit", type=int, default=None)

    utxos_parser = data_subparsers.add_parser("utxos", help="List address UTXOs")
    utxos_parser.add_argument("address", help="Bitcoin address")

    txs_parser = data_subparsers.add_parser("txs", help="List confirmed transactions")
    txs_parser.add_argument("--address", default=None)
    txs_parser.add_argument("--txid", default=None)
    txs_parser.add_argument("--from-height", type=int, default=None)
    txs_parser.add_argument("--to-height", type=int, default=None)
    txs_parser.add_argument("--from-time", type=int, default=None)
    txs_parser.add_argument("--to-time", type=int, default=None)
    txs_parser.add_argument("--offset", type=int, default=None)
    txs_parser.add_argument("--limit", type=int, default=None)

    mempool_parser = data_subparsers.add_parser("mempool", help="List mempool transactions")
    mempool_parser.add_argument("--address", default=None)
    mempool_parser.add_argument("--offset", type=int, default=None)
    mempool_parser.add_argument("--limit", type=int, default=None)

    blocks_parser = data_subparsers.add_parser("blocks", help="List blocks")
    blocks_parser.add_argument("--address", default=None)
    blocks_parser.add_argument("--block-hash", default=None)
    blocks_parser.add_argument("--has-txid", default=None)
    blocks_parser.add_argument("--from-height", type=int, default=None)
    blocks_parser.add_argument("--to-height", type=int, default=None)
    blocks_parser.add_argument("--from-time", type=int, default=None)
    blocks_parser.add_argument("--to-time", type=int, default=None)
    blocks_parser.add_argument("--offset", type=int, default=None)
    blocks_parser.add_argument("--limit", type=int, default=None)

    return parser


def build_config(args: argparse.Namespace) -> CliConfig:
    if not args.username:
        raise CliError("missing Basic Auth username: pass --username or set INDEXER_API_USERNAME")
    if not args.password:
        raise CliError("missing Basic Auth password: pass --password or set INDEXER_API_PASSWORD")

    return CliConfig(
        base_url=args.base_url,
        username=args.username,
        password=args.password,
    )


def handle_jobs(client: ApiClient, args: argparse.Namespace) -> Any:
    if args.action == "list":
        return client.get("/v1/jobs")
    if args.action == "get":
        return client.get(f"/v1/jobs/{args.job_id}")
    if args.action in {"start", "stop", "pause", "resume", "retry"}:
        return client.post(f"/v1/jobs/{args.job_id}/{args.action}")
    raise CliError(f"unsupported jobs action: {args.action}")


def handle_nodes(client: ApiClient, args: argparse.Namespace) -> Any:
    if args.action == "list":
        return client.get("/v1/nodes")
    if args.action == "health":
        return client.get(f"/v1/nodes/{args.node_id}/health")
    raise CliError(f"unsupported nodes action: {args.action}")


def handle_data(client: ApiClient, args: argparse.Namespace) -> Any:
    if args.action == "balance":
        return client.get(
            f"/v1/data/addresses/{args.address}/balance",
            query={
                "from_time": args.from_time,
                "to_time": args.to_time,
                "from_height": args.from_height,
                "to_height": args.to_height,
            },
        )
    if args.action == "balance-history":
        return client.get(
            f"/v1/data/addresses/{args.address}/balance/history",
            query={
                "from_time": args.from_time,
                "to_time": args.to_time,
                "from_height": args.from_height,
                "to_height": args.to_height,
                "offset": args.offset,
                "limit": args.limit,
            },
        )
    if args.action == "utxos":
        return client.get(f"/v1/data/addresses/{args.address}/utxos")
    if args.action == "txs":
        return client.get(
            "/v1/data/transactions",
            query={
                "address": args.address,
                "txid": args.txid,
                "from_height": args.from_height,
                "to_height": args.to_height,
                "from_time": args.from_time,
                "to_time": args.to_time,
                "offset": args.offset,
                "limit": args.limit,
            },
        )
    if args.action == "mempool":
        return client.get(
            "/v1/data/transactions/mempool",
            query={
                "address": args.address,
                "offset": args.offset,
                "limit": args.limit,
            },
        )
    if args.action == "blocks":
        return client.get(
            "/v1/data/blocks",
            query={
                "address": args.address,
                "block_hash": args.block_hash,
                "has_txid": args.has_txid,
                "from_height": args.from_height,
                "to_height": args.to_height,
                "from_time": args.from_time,
                "to_time": args.to_time,
                "offset": args.offset,
                "limit": args.limit,
            },
        )
    raise CliError(f"unsupported data action: {args.action}")


def main(argv: list[str]) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)

    try:
        config = build_config(args)
        client = ApiClient(config)

        if args.resource == "jobs":
            payload = handle_jobs(client, args)
        elif args.resource == "nodes":
            payload = handle_nodes(client, args)
        elif args.resource == "data":
            payload = handle_data(client, args)
        else:
            raise CliError(f"unsupported resource: {args.resource}")
    except CliError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1

    print(json.dumps(payload, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

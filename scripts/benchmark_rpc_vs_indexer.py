#!/usr/bin/env python3
"""Compare Bitcoin Blockchain Indexer HTTP API with Bitcoin Core JSON-RPC.

The script intentionally uses only the Python standard library so it can be
started on a clean machine without installing benchmark dependencies.
"""

from __future__ import annotations

import argparse
import base64
import csv
import json
import math
import os
import statistics
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any


@dataclass(frozen=True)
class Measurement:
    latency_ms: float
    ok: bool
    error: str | None = None


def percentile(values: list[float], percent: int) -> float | None:
    if not values:
        return None
    ordered = sorted(values)
    index = max(0, math.ceil(percent / 100 * len(ordered)) - 1)
    return ordered[index]


def auth_header(config: dict[str, Any]) -> dict[str, str]:
    username = config.get("username")
    password = config.get("password")
    password_env = config.get("password_env")
    if password_env:
        password = os.environ.get(password_env, password)
    if not username and not password:
        return {}
    token = base64.b64encode(f"{username or ''}:{password or ''}".encode("utf-8"))
    return {"Authorization": f"Basic {token.decode('ascii')}"}


def format_value(value: Any, variables: dict[str, Any]) -> Any:
    if isinstance(value, str):
        return value.format(**variables)
    if isinstance(value, list):
        return [format_value(item, variables) for item in value]
    if isinstance(value, dict):
        return {key: format_value(item, variables) for key, item in value.items()}
    return value


def http_get(base_url: str, path: str, headers: dict[str, str], timeout_sec: float) -> None:
    request = urllib.request.Request(
        f"{base_url.rstrip('/')}{path}",
        headers={"Accept": "application/json", **headers},
        method="GET",
    )
    with urllib.request.urlopen(request, timeout=timeout_sec) as response:
        response.read()
        if response.status >= 400:
            raise RuntimeError(f"HTTP {response.status}")


def json_rpc(
    rpc_url: str,
    method: str,
    params: list[Any],
    headers: dict[str, str],
    timeout_sec: float,
    request_id: str,
) -> None:
    payload = json.dumps(
        {
            "jsonrpc": "1.0",
            "id": request_id,
            "method": method,
            "params": params,
        }
    ).encode("utf-8")
    request = urllib.request.Request(
        rpc_url,
        data=payload,
        headers={"Content-Type": "application/json", **headers},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=timeout_sec) as response:
        body = json.loads(response.read().decode("utf-8"))
        if response.status >= 400:
            raise RuntimeError(f"HTTP {response.status}")
        if body.get("error"):
            raise RuntimeError(json.dumps(body["error"], ensure_ascii=False))


def measure_once(
    target_name: str,
    endpoint: dict[str, Any],
    config: dict[str, Any],
    variables: dict[str, Any],
    timeout_sec: float,
    request_id: str,
) -> Measurement:
    started_at = time.perf_counter_ns()
    try:
        if target_name == "indexer":
            indexer = config["indexer"]
            path = format_value(endpoint["path"], variables)
            http_get(indexer["base_url"], path, auth_header(indexer), timeout_sec)
        elif target_name == "bitcoin_core":
            core = config["bitcoin_core"]
            method = format_value(endpoint["method"], variables)
            params = format_value(endpoint.get("params", []), variables)
            json_rpc(core["rpc_url"], method, params, auth_header(core), timeout_sec, request_id)
        else:
            raise ValueError(f"Unknown target: {target_name}")
        return Measurement((time.perf_counter_ns() - started_at) / 1_000_000, True)
    except (OSError, urllib.error.URLError, urllib.error.HTTPError, RuntimeError, ValueError) as exc:
        return Measurement((time.perf_counter_ns() - started_at) / 1_000_000, False, str(exc))


def summarize_measurements(scenario_name: str, target_name: str, items: list[Measurement]) -> dict[str, Any]:
    successful = [item.latency_ms for item in items if item.ok]
    errors = [item.error for item in items if not item.ok and item.error]
    total_latency_sec = sum(item.latency_ms for item in items) / 1000
    return {
        "scenario": scenario_name,
        "target": target_name,
        "requests": len(items),
        "successful_requests": len(successful),
        "success_rate": round(len(successful) / len(items), 4) if items else 0,
        "avg_ms": round(statistics.fmean(successful), 3) if successful else None,
        "min_ms": round(min(successful), 3) if successful else None,
        "p50_ms": round(percentile(successful, 50), 3) if successful else None,
        "p95_ms": round(percentile(successful, 95), 3) if successful else None,
        "p99_ms": round(percentile(successful, 99), 3) if successful else None,
        "max_ms": round(max(successful), 3) if successful else None,
        "throughput_rps": round(len(successful) / total_latency_sec, 3) if total_latency_sec else None,
        "first_error": errors[0] if errors else None,
    }


def run_target(
    target_name: str,
    endpoint: dict[str, Any],
    scenario: dict[str, Any],
    config: dict[str, Any],
    defaults: dict[str, Any],
) -> list[Measurement]:
    warmup = int(scenario.get("warmup", defaults.get("warmup", 5)))
    iterations = int(scenario.get("iterations", defaults.get("iterations", 100)))
    timeout_sec = float(scenario.get("timeout_sec", defaults.get("timeout_sec", 10)))
    variables = scenario.get("variables", {})

    for index in range(warmup):
        measure_once(target_name, endpoint, config, variables, timeout_sec, f"{scenario['name']}-warmup-{index}")

    return [
        measure_once(target_name, endpoint, config, variables, timeout_sec, f"{scenario['name']}-{index}")
        for index in range(iterations)
    ]


def compare_summaries(indexer: dict[str, Any] | None, core: dict[str, Any] | None) -> dict[str, Any] | None:
    if not indexer or not core:
        return None
    if not indexer.get("p50_ms") or not core.get("p50_ms"):
        return None
    return {
        "scenario": indexer["scenario"],
        "indexer_p50_ms": indexer["p50_ms"],
        "bitcoin_core_p50_ms": core["p50_ms"],
        "p50_speedup": round(core["p50_ms"] / indexer["p50_ms"], 3),
        "indexer_p95_ms": indexer["p95_ms"],
        "bitcoin_core_p95_ms": core["p95_ms"],
        "p95_speedup": round(core["p95_ms"] / indexer["p95_ms"], 3)
        if indexer.get("p95_ms") and core.get("p95_ms")
        else None,
    }


def write_csv(path: Path, rows: list[dict[str, Any]]) -> None:
    if not rows:
        return
    keys = list(rows[0].keys())
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=keys)
        writer.writeheader()
        writer.writerows(rows)


def main() -> int:
    parser = argparse.ArgumentParser(description="Benchmark Bitcoin Blockchain Indexer against Bitcoin Core RPC.")
    parser.add_argument("--config", required=True, help="Path to benchmark JSON config.")
    parser.add_argument("--output-dir", default="reports/benchmarks", help="Directory for JSON and CSV reports.")
    parser.add_argument("--json-name", default="rpc_vs_indexer.json", help="JSON report file name.")
    parser.add_argument("--csv-name", default="rpc_vs_indexer.csv", help="CSV summary file name.")
    args = parser.parse_args()

    config_path = Path(args.config)
    config = json.loads(config_path.read_text(encoding="utf-8"))
    defaults = config.get("defaults", {})

    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    summaries: list[dict[str, Any]] = []
    comparisons: list[dict[str, Any]] = []

    for scenario in config.get("scenarios", []):
        if scenario.get("enabled", True) is False:
            continue

        scenario_summaries: dict[str, dict[str, Any]] = {}
        for target_name in ("indexer", "bitcoin_core"):
            endpoint = scenario.get(target_name)
            if not endpoint:
                continue
            measurements = run_target(target_name, endpoint, scenario, config, defaults)
            summary = summarize_measurements(scenario["name"], target_name, measurements)
            summaries.append(summary)
            scenario_summaries[target_name] = summary

        comparison = compare_summaries(
            scenario_summaries.get("indexer"),
            scenario_summaries.get("bitcoin_core"),
        )
        if comparison:
            comparisons.append(comparison)

    report = {
        "source_config": str(config_path),
        "generated_at_unix": int(time.time()),
        "summaries": summaries,
        "comparisons": comparisons,
    }
    (output_dir / args.json_name).write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    write_csv(output_dir / args.csv_name, summaries)

    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

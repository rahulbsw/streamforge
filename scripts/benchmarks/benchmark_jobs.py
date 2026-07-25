#!/usr/bin/env python3
"""Barrier and progress-file utilities for the sustained benchmark."""

from __future__ import annotations

import argparse
from typing import Any

from benchmark_job_common import atomic_json, monotonic_ns, read_json


def write_barrier(args: argparse.Namespace) -> None:
    atomic_json(
        args.path,
        {
            "start_monotonic_ns": monotonic_ns(),
            "duration_seconds": args.duration_seconds,
        },
    )


def json_field(args: argparse.Namespace) -> None:
    value: Any = read_json(args.path)
    for component in args.field.split("."):
        value = value[component]
    if isinstance(value, bool):
        print(str(value).lower())
    else:
        print(value)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    barrier = subparsers.add_parser("write-barrier")
    barrier.add_argument("--path", required=True)
    barrier.add_argument("--duration-seconds", type=int, required=True)
    barrier.set_defaults(func=write_barrier)
    field = subparsers.add_parser("json-field")
    field.add_argument("--path", required=True)
    field.add_argument("--field", required=True)
    field.set_defaults(func=json_field)
    return parser


def main() -> None:
    args = build_parser().parse_args()
    args.func(args)


if __name__ == "__main__":
    main()

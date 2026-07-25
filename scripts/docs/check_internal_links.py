#!/usr/bin/env python3
"""Fail when a generated GitHub Pages site contains a missing internal target."""

from __future__ import annotations

import argparse
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urlsplit


class LinkCollector(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.links: list[str] = []

    def handle_starttag(
        self, tag: str, attrs: list[tuple[str, str | None]]
    ) -> None:
        attribute = "href" if tag in {"a", "link"} else "src"
        if tag not in {"a", "link", "img", "script", "source"}:
            return
        for name, value in attrs:
            if name == attribute and value:
                self.links.append(value)


def candidate_targets(path: Path) -> list[Path]:
    if path.suffix:
        return [path]
    return [path, path.with_suffix(".html"), path / "index.html"]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("site_root", type=Path)
    parser.add_argument("--baseurl", default="")
    args = parser.parse_args()

    site_root = args.site_root.resolve()
    baseurl = "/" + args.baseurl.strip("/") if args.baseurl.strip("/") else ""
    failures: list[str] = []

    for page in sorted(site_root.rglob("*.html")):
        collector = LinkCollector()
        collector.feed(page.read_text(encoding="utf-8"))

        for raw_link in collector.links:
            if "{{" in raw_link or "{%" in raw_link:
                failures.append(f"{page.relative_to(site_root)}: unresolved {raw_link}")
                continue

            parsed = urlsplit(raw_link)
            if parsed.scheme or parsed.netloc or not parsed.path:
                continue

            decoded_path = unquote(parsed.path)
            if decoded_path.startswith("/"):
                if baseurl and (
                    decoded_path == baseurl
                    or decoded_path.startswith(f"{baseurl}/")
                ):
                    decoded_path = decoded_path[len(baseurl) :] or "/"
                target = site_root / decoded_path.lstrip("/")
            else:
                target = page.parent / decoded_path

            resolved_targets = [candidate.resolve() for candidate in candidate_targets(target)]
            if any(
                candidate.exists()
                and (candidate == site_root or site_root in candidate.parents)
                for candidate in resolved_targets
            ):
                continue

            failures.append(
                f"{page.relative_to(site_root)} -> {raw_link}"
            )

    if failures:
        print("Broken internal links:")
        for failure in failures:
            print(f"  {failure}")
        return 1

    print("Internal link validation passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

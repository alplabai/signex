#!/usr/bin/env python3
"""Apply .github/milestones.yml to the repository's milestones.

Mirrors what EndBug/label-sync does for labels.yml, which has no
equivalent action for milestones. Creates missing milestones, updates
drifted title/description/state, and reports anything on GitHub that the
file does not declare.

Deliberately never deletes. A milestone carrying issues is a scheduling
commitment; removing it silently unassigns every issue on it. Drift is
reported and left for a human.

Usage:
    sync_milestones.py --repo owner/name [--dry-run]

Reads GITHUB_TOKEN from the environment.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.error
import urllib.request

import yaml

API = "https://api.github.com"


def request(method: str, url: str, token: str, payload: dict | None = None) -> object:
    body = json.dumps(payload).encode() if payload is not None else None
    req = urllib.request.Request(url, data=body, method=method)
    req.add_header("Authorization", f"Bearer {token}")
    req.add_header("Accept", "application/vnd.github+json")
    req.add_header("X-GitHub-Api-Version", "2022-11-28")
    if body is not None:
        req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req) as resp:
            return json.load(resp) if resp.status != 204 else None
    except urllib.error.HTTPError as err:
        detail = err.read().decode(errors="replace")
        raise SystemExit(f"::error::{method} {url} failed: {err.code} {detail}") from err


def fetch_existing(repo: str, token: str) -> list[dict]:
    milestones: list[dict] = []
    page = 1
    while True:
        batch = request(
            "GET", f"{API}/repos/{repo}/milestones?state=all&per_page=100&page={page}", token
        )
        if not batch:
            return milestones
        milestones.extend(batch)
        page += 1


def load_desired(path: str) -> list[dict]:
    with open(path, encoding="utf-8") as handle:
        entries = yaml.safe_load(handle) or []

    seen: set[str] = set()
    for entry in entries:
        title = entry.get("title")
        if not title:
            raise SystemExit(f"::error::milestone entry missing a title: {entry!r}")
        if title in seen:
            raise SystemExit(f"::error::duplicate milestone title: {title}")
        if entry.get("state", "open") not in ("open", "closed"):
            raise SystemExit(f"::error::{title}: state must be 'open' or 'closed'")
        if not isinstance(entry.get("aliases", []), list):
            raise SystemExit(f"::error::{title}: aliases must be a list")
        seen.add(title)

    # An alias naming a title this file also declares would make the
    # rename target ambiguous — which of the two wins depends on dict
    # ordering, and the loser gets silently orphaned.
    for entry in entries:
        for alias in entry.get("aliases", []):
            if alias in seen:
                raise SystemExit(
                    f"::error::{entry['title']}: alias '{alias}' is itself a declared title"
                )
    return entries


def resolve(entry: dict, existing: dict[str, dict]) -> tuple[dict | None, str | None]:
    """Find the milestone this entry refers to, by title or by alias."""
    title = entry["title"]
    if title in existing:
        return existing[title], None

    for alias in entry.get("aliases", []):
        if alias in existing:
            return existing[alias], alias
    return None, None


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", required=True)
    parser.add_argument("--file", default=".github/milestones.yml")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    token = os.environ.get("GITHUB_TOKEN")
    if not token:
        raise SystemExit("::error::GITHUB_TOKEN is not set")

    desired = load_desired(args.file)
    existing = {m["title"]: m for m in fetch_existing(args.repo, token)}

    created = updated = unchanged = 0

    for entry in desired:
        title = entry["title"]
        want = {
            "title": title,
            "description": entry.get("description", ""),
            "state": entry.get("state", "open"),
        }
        current, matched_alias = resolve(entry, existing)

        if current is None:
            print(f"create   {title}")
            if not args.dry_run:
                request("POST", f"{API}/repos/{args.repo}/milestones", token, want)
            created += 1
            continue

        drift = {
            key: value
            for key, value in want.items()
            if (current.get(key) or "") != value
        }
        if not drift:
            unchanged += 1
            continue

        if matched_alias:
            print(f"rename   {matched_alias!r} -> {title!r}  ({', '.join(sorted(drift))})")
        else:
            print(f"update   {title}  ({', '.join(sorted(drift))})")
        if not args.dry_run:
            request(
                "PATCH",
                f"{API}/repos/{args.repo}/milestones/{current['number']}",
                token,
                want,
            )
        updated += 1

    # Anything on GitHub but not in the file. Never deleted — a milestone
    # with issues on it is load-bearing, and one without may simply be
    # newer than this file. Aliases count as accounted-for: a milestone
    # matched by alias was renamed above, not left behind.
    accounted = {e["title"] for e in desired}
    accounted |= {alias for e in desired for alias in e.get("aliases", [])}
    undeclared = [m for title, m in existing.items() if title not in accounted]
    for milestone in undeclared:
        attached = milestone["open_issues"] + milestone["closed_issues"]
        note = f"{attached} issue(s) attached" if attached else "no issues attached"
        print(f"::warning::milestone '{milestone['title']}' is not in {args.file} ({note})")

    print(
        f"\ncreated={created} updated={updated} unchanged={unchanged} "
        f"undeclared={len(undeclared)}"
        + ("  [dry run — nothing was written]" if args.dry_run else "")
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Turn a cargo-llvm-cov JSON summary into a Markdown coverage report.

Usage: python3 scripts/coverage_summary.py <coverage.json> [<base.json>]

Run it from the repository's root (or set COV_ROOT to it): file paths are
shown from there.

<coverage.json> is `cargo llvm-cov report --json --summary-only` output.
With <base.json> (the same report for the base branch), each crate's line
coverage also shows the change. Prints the report on stdout: a line saying
which commit was measured and when, a table per crate, then a collapsed
table per file.

The commit line uses these environment variables, set by CI; any missing
one is left out:
  COV_COMMIT    the commit measured (full SHA)
  COV_REPO_URL  the repository's page, to link the commit
  COV_RUN_URL   the CI run's page

Adapted from raptaboard's scripts/coverage_summary.py. Written by Claude.
"""

import datetime
import json
import os
import pathlib
import sys

# Marks the PR comment so CI updates it instead of adding a new one.
MARKER = "<!-- raptaboard-examples-coverage -->"
KINDS = ("lines", "functions", "regions")


def load(path: str) -> dict:
    """{relative file path: summary} from an llvm-cov JSON export."""
    data = json.loads(pathlib.Path(path).read_text())
    files = {}
    root = os.environ.get("COV_ROOT") or os.getcwd()
    root = root.replace("\\", "/").rstrip("/") + "/"
    for f in data["data"][0]["files"]:
        name = f["filename"].replace("\\", "/")
        # Paths are absolute; keep them from the repository's root on.
        files[name[len(root):] if name.startswith(root) else name] = f["summary"]
    return files


def crate_of(path: str) -> str:
    """The plugin a file is in: its top folder (`number/src/lib.rs`)."""
    parts = path.split("/")
    return parts[0] if len(parts) > 1 else "(other)"


def add(totals: dict, summary: dict) -> None:
    for k in KINDS:
        t = totals.setdefault(k, [0, 0])
        t[0] += summary[k]["covered"]
        t[1] += summary[k]["count"]


def pct(covered: int, count: int) -> float | None:
    return 100.0 * covered / count if count else None


def fmt(covered: int, count: int) -> str:
    p = pct(covered, count)
    return "–" if p is None else f"{p:.1f}% ({covered}/{count})"


def by_crate(files: dict) -> dict:
    crates: dict = {}
    for path, summary in files.items():
        add(crates.setdefault(crate_of(path), {}), summary)
    return crates


def delta(now: list, base: list | None) -> str:
    if base is None:
        return "new"
    a, b = pct(*now), pct(*base)
    if a is None or b is None:
        return ""
    d = a - b
    if abs(d) < 0.05:
        return "±0.0"
    return f"{d:+.1f}"


def measured_line(
    commit: str | None,
    repo_url: str | None,
    when: datetime.datetime,
    run_url: str | None,
) -> str:
    """Which commit was measured and when, so a PR comment that is edited
    in place on every push shows how fresh it is:
    "Measured commit [`abc1234`](...) at 2026-11-18 21:20 UTC ([CI run](...))."
    """
    line = "Measured"
    if commit:
        short = f"`{commit[:7]}`"
        if repo_url:
            short = f"[{short}]({repo_url}/commit/{commit})"
        line += f" commit {short}"
    line += f" at {when.strftime('%Y-%m-%d %H:%M')} UTC"
    if run_url:
        line += f" ([CI run]({run_url}))"
    return line + "."


def report(files: dict, base: dict | None, measured: str | None = None) -> str:
    crates = by_crate(files)
    base_crates = by_crate(base) if base is not None else None
    total: dict = {}
    for summary in files.values():
        add(total, summary)

    out = [MARKER, "## Coverage", ""]
    if measured:
        out += [measured, ""]
    line = f"**{fmt(*total['lines'])}** of lines covered"
    if base is not None:
        base_total: dict = {}
        for summary in base.values():
            add(base_total, summary)
        line += f" ({delta(total['lines'], base_total.get('lines'))} against the base branch)"
    out += [line + ".", ""]

    header = "| Crate | Lines | Functions | Regions |"
    if base_crates is not None:
        header += " Lines vs base |"
    out += [header, "|" + " --- |" * (header.count("|") - 1)]
    for crate in sorted(crates):
        t = crates[crate]
        row = f"| `{crate}` | " + " | ".join(fmt(*t[k]) for k in KINDS) + " |"
        if base_crates is not None:
            b = base_crates.get(crate)
            row += f" {delta(t['lines'], b['lines'] if b else None)} |"
        out.append(row)

    out += ["", "<details><summary>By file</summary>", ""]
    out += ["| File | Lines | Functions |", "| --- | --- | --- |"]
    for path in sorted(files):
        s = files[path]
        cells = [fmt(s[k]["covered"], s[k]["count"]) for k in ("lines", "functions")]
        out.append(f"| `{path}` | {cells[0]} | {cells[1]} |")
    out += ["", "</details>", ""]
    out.append(
        "Each plugin's unit tests (in `src/`), run natively; files in `tests/` "
        "aren't counted. The harness tests (`tests/`) run the plugins as "
        "WebAssembly, which this doesn't measure, so the real coverage is higher."
    )
    return "\n".join(out) + "\n"


def main() -> int:
    if len(sys.argv) not in (2, 3):
        print(__doc__)
        return 2
    files = load(sys.argv[1])
    base = None
    if len(sys.argv) == 3 and pathlib.Path(sys.argv[2]).is_file():
        base = load(sys.argv[2])
    measured = measured_line(
        os.environ.get("COV_COMMIT") or None,
        os.environ.get("COV_REPO_URL") or None,
        datetime.datetime.now(datetime.timezone.utc),
        os.environ.get("COV_RUN_URL") or None,
    )
    sys.stdout.write(report(files, base, measured))
    return 0


if __name__ == "__main__":
    sys.exit(main())

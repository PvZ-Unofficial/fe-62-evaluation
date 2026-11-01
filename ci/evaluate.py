"""Build once, execute independent shards, and validate complete-root reports."""
import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import statistics
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]


def read(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def save(path, value):
    Path(path).write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def request(path):
    value = read(path)
    seeds = value["seeds"]
    if len(seeds) != 300 or len(set(seeds)) != 300 or any(type(s) is not int or not 1000000 < s < 2**32 for s in seeds):
        raise ValueError("request must contain 300 distinct uint32 roots")
    shard_count(value)
    return value


def shard_count(req):
    count = req.get("shards", 6)
    if type(count) is not int or not 1 <= count <= len(req["seeds"]):
        raise ValueError("shards must be an integer between one and the root count")
    return count


def shard_seeds(req, index):
    count = shard_count(req)
    if type(index) is not int or not 0 <= index < count:
        raise ValueError("shard index outside request partition")
    seeds = req["seeds"]
    return seeds[len(seeds) * index // count:len(seeds) * (index + 1) // count]


def matrix(args):
    shards = json.dumps(list(range(shard_count(request(args.request)))))
    if "GITHUB_OUTPUT" in os.environ:
        with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
            output.write("shards=" + shards + "\n")
    else:
        print(shards)


def build(args):
    req = request(args.request)
    snapshot = read(ROOT / "SNAPSHOT.json")
    if req["strategy_commit"] != snapshot["strategy"]["source_commit"]:
        raise ValueError("request belongs to another strategy revision; allocate a fresh seed batch")
    bundle = ROOT / "bundle"
    bundle.mkdir(exist_ok=False)
    started = time.monotonic()
    framework = ROOT / "RustVsZombies"
    subprocess.run(["cargo", "build", "--locked", "-p", "rsvz-cli"], cwd=framework, check=True)
    cli = framework / "target/debug/rsvz.exe"
    command = [str(cli), "run-pe", "--script-crate", str(ROOT / "strategy"), "--release", "--pe-toolchain", "clang-thin-lto", "--build-only"]
    result = subprocess.run(command, cwd=framework, capture_output=True, text=True, encoding="utf-8", errors="replace")
    (bundle / "build.log").write_text(result.stdout + result.stderr, encoding="utf-8")
    if result.returncode:
        print((result.stdout + result.stderr)[-12000:])
        result.check_returncode()
    match = re.search(r"PE runner EXE: (.+)", result.stdout + result.stderr)
    if not match:
        raise RuntimeError("runner build returned no executable")
    exe = Path(match[1].strip())
    shutil.copy2(exe, bundle / "runner.exe")
    for dll in exe.parent.glob("*.dll"):
        shutil.copy2(dll, bundle / dll.name)
    shutil.copy2(ROOT / "strategy/run.py", bundle / "run.py")
    save(bundle / "request.json", req)
    save(bundle / "parameters.json", read(ROOT / "ci/parameters.json"))
    save(bundle / "build.json", {
        "commit": os.environ.get("GITHUB_SHA", subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()),
        "snapshot": snapshot,
        "runner_sha256": digest(bundle / "runner.exe"),
        "seconds": time.monotonic() - started,
        "files": {f.name: digest(f) for f in bundle.iterdir() if f.is_file()},
    })


def parse_rounds(folder):
    rounds = {}
    for file in folder.glob("trace.*.jsonl"):
        for line in file.open(encoding="utf-8"):
            message = json.loads(line).get("payload", {}).get("message", "")
            if message.startswith("six_two_trial "):
                fields = dict(re.findall(r"(\w+)=([^ ]+)", message))
                rounds.setdefault(int(fields["seed"]), []).append({
                    "round": int(fields["round"]), "outcome": fields["outcome"],
                    "end_sun": int(fields["end_sun"]),
                })
    return rounds


def validate(roots, rounds, seeds):
    if len(roots) != len(seeds) or {r["seed"] for r in roots} != set(seeds):
        raise ValueError("missing or duplicate root results")
    for r in roots:
        if r["status"] != "failed" or r["cause"] in ("timeout", "unreported_or_not_started"):
            raise ValueError("incomplete/censored root is not a valid terminal failure")
        chain = sorted(rounds.get(r["seed"], []), key=lambda v: v["round"])
        if [v["round"] for v in chain] != list(range(1, r["n"] + 2)):
            raise ValueError("missing or duplicated round balance")
        if any(v["outcome"] != "success" for v in chain[:-1]) or chain[-1]["outcome"] != r["cause"]:
            raise ValueError("root and round outcomes disagree")
        if chain[-1]["end_sun"] != r["sun"]:
            raise ValueError("terminal sun mismatch")


def shard(args):
    bundle = ROOT / "bundle"
    info = read(bundle / "build.json")
    for name, expected in info["files"].items():
        if digest(bundle / name) != expected:
            raise ValueError(f"bundle checksum mismatch: {name}")
    req = request(bundle / "request.json")
    seeds = shard_seeds(req, args.index)
    folder = ROOT / "outputs" / f"shard-{args.index}"
    folder.mkdir(parents=True, exist_ok=False)
    seedfile = folder / "seeds.txt"
    seedfile.write_text("".join(f"{s}\n" for s in seeds), encoding="utf-8")
    # run.py uses its sibling framework directory only as the runner cwd.
    (ROOT / "RustVsZombies").mkdir(exist_ok=True)
    command = [sys.executable, str(bundle / "run.py"), "--runner", str(bundle / "runner.exe"),
               "--output", str(folder / "run"), "--seed-file", str(seedfile), "--trials", str(len(seeds)),
               "--workers", "4", "--rounds", "0", "--trace", "0", "--detail", "0", "--policy", "rule"]
    for key, value in read(bundle / "parameters.json").items():
        command += ["--env", f"{key}={value}"]
    subprocess.run(command, cwd=ROOT, check=True)
    roots = read(folder / "run/results.json")["roots"]
    rounds = parse_rounds(folder / "run")
    validate(roots, rounds, seeds)
    save(folder / "report.json", {"shard": args.index, "build": info, "request": req,
         "roots": roots, "rounds": rounds, "timing": read(folder / "run/timing.json")})


def summarize(args):
    files = list((ROOT / "outputs").rglob("report.json"))
    if not files:
        raise ValueError("no successful shards")
    reports = [read(f) for f in files]
    count = shard_count(reports[0]["request"])
    if len(files) != count:
        raise ValueError(f"expected {count} successful shards, got {len(files)}")
    if sorted(r["shard"] for r in reports) != list(range(count)):
        raise ValueError("duplicated shard")
    first = reports[0]
    roots, rounds = [], {}
    for r in reports:
        if r["build"] != first["build"] or r["request"] != first["request"]:
            raise ValueError("mixed revisions, runner binaries or seed batches")
        seeds = shard_seeds(r["request"], r["shard"])
        rr = {int(k): v for k, v in r["rounds"].items()}
        validate(r["roots"], rr, seeds)
        roots.extend(r["roots"])
        rounds.update(rr)
    validate(roots, rounds, first["request"]["seeds"])
    costs = []
    for chain in rounds.values():
        previous = 100000
        for row in sorted(chain, key=lambda v: v["round"]):
            if row["outcome"] == "success":
                costs.append(previous - row["end_sun"])
            previous = row["end_sun"]
    counts = [r["n"] for r in roots]
    summary = {"count": 300, "first": sum(n > 0 for n in counts),
        "first_rate": sum(n > 0 for n in counts) / 300, "mean": statistics.mean(counts),
        "median": statistics.median(counts), "sd": statistics.stdev(counts),
        "successful_rounds": sum(counts), "net_sun_per_successful_round": statistics.mean(costs) if costs else None,
        "causes": dict(sorted(Counter(f"{r['cause']}:row{r['row']}:kind{r['kind']}" for r in roots).items())),
        "initial_sun": 100000, "sun_limit": 2147483647,
        "build_seconds": first["build"]["seconds"],
        "shard_seconds": {str(r["shard"]): r["timing"]["seconds"] for r in reports},
        "build": first["build"], "request": first["request"]}
    dest = ROOT / "results"
    dest.mkdir(exist_ok=True)
    save(dest / "summary.json", summary)
    save(dest / "roots.json", sorted(roots, key=lambda r: r["seed"]))
    save(dest / "rounds.json", rounds)
    text = f"# {first['request']['label']} — 300 roots\n\nCommit: `{first['build']['commit']}`\n\n"
    text += f"| Metric | Value |\n|---|---:|\n| First clear | {summary['first']}/300 ({summary['first_rate']:.2%}) |\n"
    text += f"| Mean consecutive clears | {summary['mean']:.3f} |\n| Median | {summary['median']} |\n| Net sun / successful round | {summary['net_sun_per_successful_round']} |\n"
    text += f"| Build seconds | {summary['build_seconds']:.1f} |\n| Slowest shard seconds | {max(summary['shard_seconds'].values()):.1f} |\n\n"
    text += "Failure causes count terminal roots, not all threats. Net sun includes production.\n\n| Cause / row / kind | Roots |\n|---|---:|\n"
    for cause, count in summary["causes"].items():
        text += f"| {cause} | {count} |\n"
    (dest / "summary.md").write_text(text, encoding="utf-8")
    if "GITHUB_STEP_SUMMARY" in os.environ:
        with open(os.environ["GITHUB_STEP_SUMMARY"], "a", encoding="utf-8") as output:
            output.write(text)
    print(text)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="mode", required=True)
    sub.add_parser("build").add_argument("--request", required=True)
    sub.add_parser("matrix").add_argument("--request", required=True)
    sub.add_parser("shard").add_argument("--index", type=int, required=True)
    sub.add_parser("summarize")
    args = parser.parse_args()
    {"build": build, "matrix": matrix, "shard": shard, "summarize": summarize}[args.mode](args)

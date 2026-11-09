#!/usr/bin/env python3
"""Run the full C++/Rust compatibility checks after building YaFF with tests."""
import argparse
import os
from pathlib import Path
import subprocess
from run import ROOT, build_oracle


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build-dir", type=Path, default=ROOT / "build/local")
    parser.add_argument("--cargo", default=os.environ.get("CARGO", "cargo"))
    parser.add_argument("--fuzz-cases", type=int, default=20000)
    args = parser.parse_args()
    build = args.build_dir.resolve()
    env = dict(os.environ)
    env["YAFF_PROTOC_GEN_RUST"] = str(build / "src/protoc-plugin/protoc-gen-yaff-rust")
    env["YAFF_CPP_INTEROP"] = str(build / "tests/interop/yaff_generated_interop")
    env["PROTOC"] = (build / "tests/interop/protoc-path.txt").read_text().strip()
    output = ROOT / "build/interop-results"
    output.mkdir(parents=True, exist_ok=True)

    def run(command, cwd=ROOT):
        subprocess.run(list(map(str, command)), cwd=cwd, env=env, check=True)

    oracle = build_oracle()
    run([oracle, "check", ROOT / "tests/interop/fixtures"])
    run([oracle, "write", output / "cpp"])
    for fixture in (ROOT / "tests/interop/fixtures").glob("*.bin"):
        if fixture.read_bytes() != (output / "cpp" / fixture.name).read_bytes():
            raise RuntimeError("C++ golden bytes changed: " + fixture.name)
    run([args.cargo, "test", "--offline", "--workspace"], ROOT / "rust")
    run([args.cargo, "run", "--offline", "-p", "yaff", "--example", "wire_corpus", "--", output / "rust"], ROOT / "rust")
    run([oracle, "check", output / "rust"])
    seeds = []
    for layout in ("flat", "sparse"):
        path = output / (layout + ".bin")
        run([env["YAFF_CPP_INTEROP"], "write-" + layout, path])
        seeds.append(path)
    run([args.cargo, "run", "--offline", "-p", "yaff-interop", "--example", "fuzz_reader", "--", args.fuzz_cases, *seeds], ROOT / "rust")
    print("All C++/Rust compatibility checks passed")


if __name__ == "__main__":
    main()

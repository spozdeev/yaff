#!/usr/bin/env python3
"""Build the header-only C++ oracle; no CMake, protobuf or third-party packages."""
import argparse
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
BUILD = ROOT / "build" / "interop-header"


def build_oracle():
    (BUILD / "yaff").mkdir(parents=True, exist_ok=True)
    version = (ROOT / "VERSION").read_text().strip()
    major, minor, patch = map(int, version.split("."))
    header = (ROOT / "include/yaff/version.h.in").read_text()
    for key, value in {
        "PROJECT_VERSION_MAJOR": major, "PROJECT_VERSION_MINOR": minor,
        "PROJECT_VERSION_PATCH": patch, "PROJECT_VERSION": version,
        "YAFF_VERSION_NUMBER": major * 10000 + minor * 100 + patch,
    }.items():
        header = header.replace("@" + key + "@", str(value))
    (BUILD / "yaff/version.h").write_text(header)
    executable = BUILD / "oracle"
    subprocess.run([os.environ.get("CXX", "c++"), "-std=c++20", "-O2",
                    "-I" + str(BUILD), "-I" + str(ROOT / "include"),
                    str(ROOT / "tests/interop/oracle.cpp"), "-o", str(executable)], check=True)
    return executable


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["write", "check"])
    parser.add_argument("directory", nargs="?", type=Path, default=ROOT / "tests/interop/fixtures")
    args = parser.parse_args()
    subprocess.run([str(build_oracle()), args.action, str(args.directory.resolve())], check=True)

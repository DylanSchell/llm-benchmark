#!/usr/bin/env python3
"""Compute the Gradle wrapper distribution-cache directory name for a URL.

Gradle's PathAssembler derives the directory under
``~/.gradle/wrapper/dists/<distribution-name>/<hash>/`` by taking the MD5 of
the full distribution URL and rendering it in base 36.

Reproducing that here lets the runner image pre-seed the wrapper cache without
hardcoding a magic value that silently goes stale the moment the Exercism Java
track bumps its wrapper (in which case ``./gradlew`` would quietly fall back to
downloading the distribution at test time).

Usage: gradle-dist-hash.py <distribution-url>
"""

import hashlib
import sys

DIGITS = "0123456789abcdefghijklmnopqrstuvwxyz"


def base36(value):
    if value == 0:
        return "0"
    out = ""
    while value > 0:
        value, rem = divmod(value, 36)
        out = DIGITS[rem] + out
    return out


def main(argv):
    if len(argv) != 2:
        print("usage: {} <distribution-url>".format(argv[0]), file=sys.stderr)
        return 2
    digest = hashlib.md5(argv[1].encode("utf-8")).digest()
    print(base36(int.from_bytes(digest, "big")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))

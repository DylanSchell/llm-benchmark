# Exercise Selection

The benchmark embeds a curated subset of [Exercism](https://exercism.org) practice exercises.
What ships is declared in `exercises.manifest.yaml`: each track lists the exercises to
**include**, and every other exercise under that track's `exercises/practice/` directory is
pruned. The per-track `exclude:` lists are currently empty, so exclusion is implicit — an
exercise is pruned when it is not included.

This is a generated report describing the current selection; `exercises.manifest.yaml` is the
source of truth. Exercises are embedded at build time from the Exercism track commits pinned
in the manifest (see [Pinned sources](#pinned-sources)).

## Summary

| Track | Upstream (at pin) | Included | Excluded | Excluded % |
| --- | ---: | ---: | ---: | ---: |
| cpp | 82 | 26 | 56 | 68% |
| go | 122 | 39 | 83 | 68% |
| java | 135 | 47 | 88 | 65% |
| javascript | 131 | 49 | 82 | 63% |
| python | 133 | 34 | 99 | 74% |
| rust | 102 | 30 | 72 | 71% |
| **Total** | **705** | **225** | **480** | **68%** |

Concept exercises are out of scope; only `exercises/practice` is considered.

## Shared "too easy" core

33 exercises are pruned by all six tracks:

anagram, armstrong-numbers, atbash-cipher, binary-search, bob, collatz-conjecture,
difference-of-squares, etl, grains, hamming, hello-world, isbn-verifier, isogram,
largest-series-product, leap, luhn, matching-brackets, minesweeper, nth-prime, pangram,
pascals-triangle, prime-factors, rail-fence-cipher, raindrops, reverse-string,
rna-transcription, roman-numerals, rotational-cipher, run-length-encoding,
scrabble-score, secret-handshake, sieve, two-fer

## Language-specific exclusions

15 exercises are pruned by exactly one track while being kept elsewhere:

affine-cipher, alphametics, dot-dsl, food-chain, game-of-life, ledger, lens-person,
luhn-trait, paasio, pig-latin, rational-numbers, resistor-color-expert,
state-of-tic-tac-toe, two-bucket, word-search

## Excluded by track

### cpp — 56 of 82 pruned

acronym, anagram, armstrong-numbers, atbash-cipher, beer-song, binary, binary-search,
bob, collatz-conjecture, darts, difference-of-squares, eliuds-eggs, etl, food-chain,
grains, hamming, hello-world, hexadecimal, high-scores, isbn-verifier, isogram,
largest-series-product, leap, list-ops, luhn, matching-brackets, minesweeper, nth-prime,
nucleotide-count, pangram, pascals-triangle, pig-latin, prime-factors,
protein-translation, rail-fence-cipher, raindrops, resistor-color, resistor-color-duo,
reverse-string, rna-transcription, robot-simulator, roman-numerals, rotational-cipher,
run-length-encoding, say, scrabble-score, secret-handshake, series, sieve,
simple-linked-list, sum-of-multiples, triangle, trinary, two-bucket, two-fer, word-count

### go — 83 of 122 pruned

accumulate, acronym, all-your-base, allergies, anagram, armstrong-numbers,
atbash-cipher, bank-account, binary, binary-search, binary-search-tree, bob, change,
circular-buffer, clock, collatz-conjecture, complex-numbers, custom-set, darts, diamond,
difference-of-squares, diffie-hellman, etl, flatten-array, gigasecond, grade-school,
grains, grep, hamming, hello-world, house, isbn-verifier, isogram, knapsack,
largest-series-product, leap, linked-list, list-ops, luhn, matching-brackets, meetup,
minesweeper, nth-prime, nucleotide-count, ocr-numbers, pangram,
parallel-letter-frequency, pascals-triangle, perfect-numbers, phone-number,
prime-factors, proverb, pythagorean-triplet, queen-attack, rail-fence-cipher, raindrops,
rectangles, resistor-color, resistor-color-duo, resistor-color-trio, reverse-string,
rna-transcription, robot-name, roman-numerals, rotational-cipher, run-length-encoding,
saddle-points, scrabble-score, secret-handshake, series, sieve, simple-cipher,
space-age, spiral-matrix, state-of-tic-tac-toe, strain, sum-of-multiples, tournament,
triangle, twelve-days, two-fer, word-count, yacht

### java — 88 of 135 pruned

accumulate, acronym, allergies, anagram, armstrong-numbers, atbash-cipher, beer-song,
binary, binary-search, binary-search-tree, bob, clock, collatz-conjecture,
complex-numbers, crypto-square, darts, diamond, difference-of-squares, diffie-hellman,
dnd-character, dot-dsl, eliuds-eggs, error-handling, etl, flatten-array, game-of-life,
gigasecond, grade-school, grains, grep, hamming, hello-world, hexadecimal, high-scores,
isbn-verifier, isogram, killer-sudoku-helper, knapsack, largest-series-product, leap,
linked-list, list-ops, luhn, markdown, matching-brackets, matrix, meetup, micro-blog,
minesweeper, nth-prime, nucleotide-count, octal, pangram, parallel-letter-frequency,
pascals-triangle, perfect-numbers, prime-factors, proverb, rail-fence-cipher, raindrops,
rectangles, resistor-color, resistor-color-duo, reverse-string, rna-transcription,
robot-name, robot-simulator, roman-numerals, rotational-cipher, run-length-encoding,
saddle-points, say, scrabble-score, secret-handshake, sieve, simple-cipher, space-age,
spiral-matrix, square-root, strain, sublist, sum-of-multiples, tournament, triangle,
trinary, two-fer, word-count, yacht

### javascript — 82 of 131 pruned

accumulate, acronym, all-your-base, allergies, anagram, armstrong-numbers,
atbash-cipher, bank-account, binary-search, binary-search-tree, bob, change,
circular-buffer, clock, collatz-conjecture, crypto-square, custom-set, darts, diamond,
difference-of-squares, diffie-hellman, dnd-character, dominoes, eliuds-eggs, etl,
flatten-array, gigasecond, grains, hamming, hello-world, hexadecimal, high-scores,
isbn-verifier, isogram, kindergarten-garden, knapsack, largest-series-product, leap,
lens-person, linked-list, luhn, markdown, matching-brackets, matrix, micro-blog,
minesweeper, nth-prime, nucleotide-count, octal, pangram, pascals-triangle,
perfect-numbers, point-mutations, prime-factors, protein-translation, proverb,
pythagorean-triplet, rail-fence-cipher, raindrops, resistor-color, resistor-color-duo,
reverse-string, rna-transcription, robot-simulator, roman-numerals, rotational-cipher,
run-length-encoding, saddle-points, satellite, scrabble-score, secret-handshake, series,
sieve, simple-cipher, spiral-matrix, square-root, strain, sublist, trinary, two-fer,
word-count, yacht

### python — 99 of 133 pruned

accumulate, acronym, all-your-base, allergies, alphametics, anagram, armstrong-numbers,
atbash-cipher, bank-account, binary, binary-search, binary-search-tree, bob, change,
circular-buffer, clock, collatz-conjecture, complex-numbers, crypto-square, custom-set,
darts, diamond, difference-of-squares, diffie-hellman, dnd-character, eliuds-eggs,
error-handling, etl, flatten-array, gigasecond, grains, hamming, hello-world,
hexadecimal, high-scores, house, isbn-verifier, isogram, killer-sudoku-helper,
kindergarten-garden, knapsack, largest-series-product, leap, ledger, linked-list, luhn,
markdown, matching-brackets, matrix, meetup, minesweeper, nth-prime, ocr-numbers, octal,
palindrome-products, pangram, pascals-triangle, perfect-numbers, point-mutations,
prime-factors, protein-translation, pythagorean-triplet, queen-attack,
rail-fence-cipher, raindrops, rational-numbers, rectangles, resistor-color,
resistor-color-duo, resistor-color-expert, resistor-color-trio, reverse-string,
rna-transcription, robot-simulator, roman-numerals, rotational-cipher,
run-length-encoding, saddle-points, satellite, say, scrabble-score, secret-handshake,
series, sieve, simple-cipher, space-age, spiral-matrix, square-root, strain, sublist,
sum-of-multiples, tournament, triangle, trinary, twelve-days, two-fer, word-count,
word-search, yacht

### rust — 72 of 102 pruned

affine-cipher, all-your-base, allergies, anagram, armstrong-numbers, atbash-cipher,
beer-song, binary-search, bob, circular-buffer, clock, collatz-conjecture,
crypto-square, custom-set, diamond, difference-of-squares, diffie-hellman, dominoes,
eliuds-eggs, etl, grains, hamming, hello-world, hexadecimal, high-scores, isbn-verifier,
isogram, kindergarten-garden, knapsack, largest-series-product, leap, luhn, luhn-trait,
matching-brackets, matrix, minesweeper, nth-prime, nucleotide-count, paasio,
palindrome-products, pangram, pascals-triangle, perfect-numbers, phone-number,
prime-factors, protein-translation, proverb, pythagorean-triplet, queen-attack,
rail-fence-cipher, raindrops, rectangles, reverse-string, rna-transcription,
robot-simulator, roman-numerals, rotational-cipher, run-length-encoding, saddle-points,
scrabble-score, secret-handshake, series, sieve, simple-linked-list, space-age,
spiral-matrix, sublist, sum-of-multiples, tournament, triangle, two-fer, yacht

## File-level pruning

Within the included exercises, the manifest's prune rules remove files not needed to solve or
test an exercise:

| Rule | Files removed |
| --- | ---: |
| `**/.gitignore` | 80 |
| `**/.approaches/**` | 66 |
| `**/.articles/**` | 6 |
| `**/.meta/tests.toml` | 193 |
| `**/.meta/*.j2` | 28 |
| `**/.meta/*.tera` | 15 |
| **Total** | **388** |

2436 upstream files across the included exercises → 2048 embedded files
(388 pruned).

## Pinned sources

| Track | Repository | Commit |
| --- | --- | --- |
| cpp | https://github.com/exercism/cpp | `acc32555eab077cf786f892b6ab27a22184d81c7` |
| go | https://github.com/exercism/go | `26635e19868f0b4fbed09a986b824cd67efdbc2e` |
| java | https://github.com/exercism/java | `1123e5b3dddeb9e57b802835285a886b4428bfcc` |
| javascript | https://github.com/exercism/javascript | `dbfbeae34dd4fce7fad7afd740d075d33cc51b21` |
| python | https://github.com/exercism/python | `7bec634f5c51cce82d233ad88f7ae81a3e98242a` |
| rust | https://github.com/exercism/rust | `6c321382019fa969401b8e923a5e12ff69065561` |

All tracks are MIT licensed; see `THIRD_PARTY_NOTICES`.

## Regenerating this report

The figures are derived from the pinned upstream trees in the build cache
(`target/exercises-cache/<track>`) compared against the staged bundle
(`target/exercises-bundle`). With a warm cache (offline is fine), assemble the bundle and
diff the upstream tree against it:

```bash
LLM_BENCHMARK_EXERCISES_OFFLINE=1 cargo build -p benchmark-exercises
git -C target/exercises-cache/<track> ls-tree --name-only <commit>:exercises/practice | sort
ls target/exercises-bundle/<track>/exercises/practice | sort
```

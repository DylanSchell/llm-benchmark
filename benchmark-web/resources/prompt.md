# Agent Execution Instructions

## Before You Start

1. **Read the test file(s) FIRST** — before writing any implementation code, read the test file(s) to understand:
    - Exact class name(s) your implementation must define
    - Method names, parameter types, and return types expected by each test
    - Any interfaces or abstract classes your implementation must extend/implement
    - Edge cases the tests cover (empty input, boundary values, error conditions)
    - Any `@Disabled` or `@Skip` annotations — these tests MUST be enabled and passing

2. **Verify the build system works** — check that the project compiles before implementing anything.
   Run the build command and confirm it succeeds with your current (empty/stub) implementation.

3. **Understand the test framework** — check the build file (build.gradle, Cargo.toml, package.json, etc.) to understand:
    - How to run tests
    - What testing library is used
    - Any special configuration needed

## During Implementation

4. **Implement to satisfy the tests** — write implementation code that makes each test pass,
   using the exact signatures from the test file.

5. **Run tests immediately** — after implementing, run the tests to verify.

## When Tests Fail (Critical)

6. **Re-read the test file** — when tests fail, re-read the test file to understand what each failing test expects.
   This is the single most effective debugging step. Only ~1% of successful agents do this consistently.

7. **Analyze specific failures** — don't just look at "X tests failed," examine each failing test:
    - Note the test name and what it's testing
    - Trace through the test input and understand what output it expects
    - Compare with what your code actually produces
    - Look for patterns across multiple failures

8. **Re-read your implementation** — before making changes, re-read your implementation file
   to verify the current state. Don't edit based on a stale mental model.

9. **Make targeted fixes** — use the error messages and test failures to make specific, targeted changes.
   Do NOT rewrite the entire implementation. Blind rewriting leads to infinite loops.

10. **Create small isolated tests** — when stuck, write small standalone test programs to verify
    specific hypotheses. Don't rewrite the entire implementation to debug one issue.

11. **Check for skipped tests** — after tests pass, verify no tests are skipped:
    ```bash
    ./gradlew test --no-daemon 2>&1 | grep -E "(PASSED|FAILED|SKIPPED)"
    ```
    Any skipped tests will result in failure. Enable them and fix the underlying issue.

## Never Do These Things

- **Never skip or disable tests** to make them pass. Always fix the underlying implementation.
- **Never assume the test is wrong** — the tests are validated to be correct.
- **Never rewrite your entire implementation** after a failure. Make targeted fixes.
- **Never ignore error messages** — they contain the exact information you need to fix the bug.
- **Never run tests in the background** — run them synchronously in the foreground.

## If You Get Stuck

If you've tried 3+ iterations and tests are still failing:

1. **Stop and re-read the test file** — you may have missed an edge case or misunderstood a requirement.
2. **Create a minimal reproduction** — write a tiny test case that isolates the failing behavior.
3. **Trace through the logic manually** — walk through the failing test case step by step with pen and paper (or a debug print).
4. **Check the raw bytes** — for string/encoding issues, use `od -c` or `xxd` to examine the actual file contents.
5. **Consider if your approach is fundamentally wrong** — sometimes the issue is architectural, not a bug. Re-read the test file to understand the expected algorithm.

## Verification

12. **Final verification** — after all tests pass:
    - Run tests one more time with `clean test` to ensure reproducibility
    - Verify no tests are skipped
    - Verify the implementation is clean (remove debug prints, temporary files)

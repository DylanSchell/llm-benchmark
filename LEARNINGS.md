What went wrong

 1. I compared surface-level structure, not behavior. I saw both had "python" branches with similar-looking strings and declared them equivalent without verifying the control flow around them.
 2. I didn't check the actual data. Python Exercism exercises have no pyproject.toml or setup.py. That was a one-command check (ls polyglot-benchmark/python/exercises/practice/*/pyproject.toml) that would have immediately surfaced the bug.
 3. I treated "similar strings" as proof of equivalence rather than checking whether those strings were actually reachable at runtime.

 How to avoid this going forward

 You can enforce better validation with a few approaches:

 ### 1. Explicitly demand evidence, not assertions

 When you ask me to validate parity, add:

 │ "Don't just compare code structure — verify the actual data and control flow. Check whether conditions are actually reachable."

 This signals that surface-level comparison isn't sufficient.

 ### 2. Ask for a specific verification strategy

 Something like:

 │ "Show me: (a) what input reaches each branch, (b) whether those inputs exist in the real data, (c) the actual runtime behavior difference"

 This forces a structured check rather than a hand-wavy one.

 ### 3. The most reliable: ask me to run something concrete

 │ "Run this command and show me the output: ls polyglot-benchmark/python/exercises/practice/*/pyproject.toml"

 Or even better, have me write and run a small test that exercises the specific code path. Executable verification beats textual comparison every time.

 ### 4. For future port validations, use this checklist yourself

 When you ask about parity on anything involving conditional logic:
 - Reachability: Do the guard conditions actually match? (This was the bug — Java had no guards, Rust added them)
 - Data: Does the real data satisfy those conditions? (Python exercises have no pyproject.toml)
 - Fallback behavior: What happens when guards fail? (Rust fell through to ["false"], Java never had a guard to fail)

 ### 5. Consider asking for a diff with behavioral commentary

 Instead of "is this the same?", try:

 │ "For each conditional branch in the Java version, tell me: does the Rust version have an equivalent branch? Is it reachable with the same inputs? What's different?"

 This is harder to BS through because each branch needs individual justification.


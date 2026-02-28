# Benchmark Harness Refactoring Plan

This document tracks all refactoring work for the benchmark harness, organized by priority and phase.

---

## ✅ Phase 1: Completed (2026-02-28)

### Items Completed:
1. ✅ **Extracted ResultPersister** - Moved persistence logic from BenchmarkRunner
2. ✅ **Converted Inner Classes to Records** - LanguageExercise extracted
3. ✅ **Added Configuration Validation** - validate() methods on all config classes
4. ✅ **Created AgentFactory Interface** - Replaced reflection with factory pattern

### Documentation:
- See `REFACTORING_PHASE1.md` for detailed summary of Phase 1 changes

---

## 🚧 Phase 2: High Impact, Medium Risk (Next 2-3 weeks)

### 2.1 Refactor ReferenceAgent with Strategy Pattern ⚠️⚠️⚠️

**Priority:** HIGH  
**Estimated Effort:** 3-4 days  
**Risk:** MEDIUM - Requires careful testing of all language handlers

**Problem:**
- `ReferenceAgent.java` is ~600 lines with massive methods
- `copyReferenceImplementation()`: ~200 lines of nested if/else per language
- `copyFreshTests()`: Duplicate logic for each language
- Hard-coded paths and mixed responsibilities

**Solution:**
```java
// Create strategy interface
interface LanguageHandler {
    void copyReference(Exercise exercise, Path tempDir) throws IOException;
    void copyTests(Exercise exercise, Path source, Path dest) throws IOException;
    List<String> getTestCommand(Exercise exercise);
    void patchTests(Path tempWorkDir) throws IOException;
}

// Implementations for each language
class JavaHandler implements LanguageHandler { ... }
class GoHandler implements LanguageHandler { ... }
class JavaScriptHandler implements LanguageHandler { ... }
class PythonHandler implements LanguageHandler { ... }
class RustHandler implements LanguageHandler { ... }
class CppHandler implements LanguageHandler { ... }

// Registry in ReferenceAgent
Map<String, LanguageHandler> handlers = Map.of(
    "java", new JavaHandler(),
    "go", new GoHandler(),
    // ...
);
```

**Files to Create:**
- `src/main/java/com/benchmark/agent/LanguageHandler.java`
- `src/main/java/com/benchmark/agent/handlers/JavaHandler.java`
- `src/main/java/com/benchmark/agent/handlers/GoHandler.java`
- `src/main/java/com/benchmark/agent/handlers/JavaScriptHandler.java`
- `src/main/java/com/benchmark/agent/handlers/PythonHandler.java`
- `src/main/java/com/benchmark/agent/handlers/RustHandler.java`
- `src/main/java/com/benchmark/agent/handlers/CppHandler.java`

**Files to Modify:**
- `ReferenceAgent.java` - Use handlers instead of if/else blocks

**Test Coverage Needed:**
- Unit tests for each handler
- Integration test for file copying operations

---

### 2.2 Split BenchmarkController ⚠️

**Priority:** HIGH  
**Estimated Effort:** 1 day  
**Risk:** LOW - Controllers are stateless, easy to split

**Problem:**
- `BenchmarkController.java` has 25+ endpoints in one class (~300 lines)
- Mixed concerns: benchmark control, results, queue, exercises
- Hard to maintain and test

**Solution:** Split into focused controllers

**Files to Create:**
```
src/main/java/com/benchmark/web/controller/
├── BenchmarkController.java       (run, cancel, status endpoints)
├── ResultController.java          (list, refresh, statistics)
├── QueueController.java           (schedule, cancel, clear queue)
└── ExerciseController.java        (languages, exercises discovery)
```

**Endpoint Mapping:**

| New Controller | Endpoints |
|----------------|-----------|
| BenchmarkController | `POST /api/benchmark/run`, `GET /api/benchmark/{id}/status`, `POST /api/benchmark/{id}/cancel`, `GET /api/benchmark/{id}/stream` |
| ResultController | `GET /api/results`, `POST /api/results/refresh`, `GET /api/recent-results`, `GET /api/stats` |
| QueueController | `GET /api/benchmark/queue`, `POST /api/benchmark/queue/schedule`, `POST /api/benchmark/queue/cancel/{id}`, `POST /api/benchmark/queue/clear` |
| ExerciseController | `GET /api/exercises`, `GET /api/languages` |

**Files to Modify:**
- Remove endpoints from `BenchmarkController.java` (current file)
- Update package imports in web layer

---

### 2.3 Extract CLI Entry Point ⚠️

**Priority:** MEDIUM  
**Estimated Effort:** 1 day  
**Risk:** LOW - Isolated change

**Problem:**
- `BenchmarkRunner.main()` mixes CLI parsing with orchestration
- ~100 lines of argument parsing in core class
- Hard to test CLI logic

**Solution:**
```java
// New class for CLI handling
public class CliEntryPoint {
    public static void main(String[] args) {
        CliArgs cliArgs = parseArguments(args);
        
        if (cliArgs.isWebMode()) {
            startWebMode(cliArgs);
        } else {
            runCliBenchmark(cliArgs);
        }
    }
    
    private static CliArgs parseArguments(String[] args) {
        // Use a library like Picocli or JCommander
        // Or keep simple manual parsing but in dedicated class
    }
}

// CLI arguments DTO
public record CliArgs(
    String configFile,
    boolean webMode,
    int webPort,
    String model,
    String resultsDir,
    String language,
    String exercise,
    String agent
) {}
```

**Files to Create:**
- `src/main/java/com/benchmark/CliEntryPoint.java`
- `src/main/java/com/benchmark/CliArgs.java` (or use existing record library)

**Files to Modify:**
- `BenchmarkRunner.java` - Remove main() method, keep only core logic
- Update Spring Boot configuration if needed

---

### 2.4 Improve Error Handling ⚠️

**Priority:** MEDIUM  
**Estimated Effort:** 1 day  
**Risk:** LOW

**Problem:**
- Empty catch blocks throughout codebase
- Generic `Exception` types
- Inconsistent error messages

**Issues Found:**
```java
// In BenchmarkService.java
} catch (Exception e) {
    processingItem = false;  // No logging!
}

// In ResultPersister.java  
} catch (IOException e) {
    logger.error("Failed to save...");
    return null;  // Silent failure
}
```

**Solution:**
- Replace empty catch blocks with proper logging
- Create custom exception types:
  - `BenchmarkExecutionException`
  - `ExerciseNotFoundException`
  - `DockerExecutionException`
- Add validation error messages

**Files to Modify:**
- `BenchmarkService.java`
- `ResultPersister.java`
- `ReferenceAgent.java`
- `DockerClient.java`

**Files to Create:**
- `src/main/java/com/benchmark/exception/BenchmarkExecutionException.java`
- `src/main/java/com/benchmark/exception/ExerciseNotFoundException.java`
- `src/main/java/com/benchmark/exception/DockerExecutionException.java`

---

### 2.5 Split BenchmarkService ⚠️⚠️

**Priority:** MEDIUM  
**Estimated Effort:** 2 days  
**Risk:** MEDIUM - Complex concurrent logic

**Problem:**
- `BenchmarkService.java` has ~350 lines with mixed concerns
- Session management, queue processing, benchmark execution all in one class
- Reflection-based agent creation (now fixed with AgentFactory)
- Busy-wait loops with Thread.sleep()

**Solution:**
```java
// Split into focused services
interface BenchmarkExecutor {
    void execute(BenchmarkSession session);
}

interface SessionManager {
    BenchmarkSession createSession(String id, ...);
    BenchmarkSession getSession(String id);
    void cancelSession(String id);
}

interface QueueProcessor {
    void addQueueItem(BenchmarkQueueItem item);
    void processQueue();
}
```

**Files to Create:**
- `src/main/java/com/benchmark/web/service/BenchmarkExecutor.java`
- `src/main/java/com/benchmark/web/service/SessionManager.java`
- `src/main/java/com/benchmark/web/service/QueueProcessor.java`

**Files to Modify:**
- `BenchmarkService.java` - Delegate to new services
- Consider using Spring's `@Scheduled` instead of manual thread loop

---

### 2.6 Fix Build Warnings ⚠️

**Priority:** LOW  
**Estimated Effort:** 2 hours  
**Risk:** VERY LOW

**Issues:**
```
[WARNING] location of system modules is not set in conjunction with -source 17
  --release 17 is recommended instead of -source 17 -target 17

[INFO] ResultService.java uses unchecked or unsafe operations.
  Recompile with -Xlint:unchecked for details.
```

**Solution:**
- Update pom.xml to use `--release 17` instead of `-source 17 -target 17`
- Fix unchecked warnings in ResultService.java

**Files to Modify:**
- `pom.xml` - Change compiler plugin configuration
- `ResultService.java` - Add proper generics

---

## 🔮 Phase 3: Architectural Improvements (3-4 weeks)

### 3.1 Refactor BenchmarkResultAnalyzer ⚠️⚠️

**Priority:** MEDIUM  
**Estimated Effort:** 2 days  
**Risk:** MEDIUM - Complex parsing logic

**Problem:**
- ~350 lines with manual string parsing
- Inner classes `SimpleResult`, `BenchmarkStats` duplicate existing models
- No separation of concerns (loading, analyzing, reporting)

**Solution:**
```java
// Split into focused components
interface ResultLoader {
    List<ExerciseResult> loadResults(Path directory);
}

interface ResultAggregator {
    BenchmarkStats aggregate(List<ExerciseResult> results);
}

interface ReportGenerator {
    void generateMarkdown(BenchmarkStats stats, Path output);
}
```

**Files to Create:**
- `src/main/java/com/benchmark/report/ResultLoader.java`
- `src/main/java/com/benchmark/report/ResultAggregator.java`
- `src/main/java/com/benchmark/report/ReportGenerator.java`

**Files to Modify:**
- `BenchmarkResultAnalyzer.java` - Delegate to new components
- Remove inner classes, use existing `ExerciseResult` model

---

### 3.2 Consolidate Model Classes ⚠️⚠️

**Priority:** LOW  
**Estimated Effort:** 2 days  
**Risk:** MEDIUM - Many dependencies

**Problem:**
- ~20 model files in `com.benchmark.model` package
- Mix of records, classes, and inner classes
- Some only used for JSON parsing
- Inconsistent naming conventions

**Current Models:**
```
AssistantEntry, AssistantError, BashProgress, CacheCreation,
Container, Content, ContentListDeserializer, ContextManagement,
HookProgress, LogEntry, Message, Progress, ProgressData,
QueueOperationEntry, ServerToolUse, ServiceTier, StringContent,
SystemEntry, TextContent, ThinkingContent, ToolUseContent,
Usage, UserEntry, WaitingForTask
```

**Solution:**
- Group into logical subpackages:
  - `com.benchmark.model.agent` (Message, Usage, Content types)
  - `com.benchmark.model.exercise` (ExerciseResult, ExerciseMetadata)
  - `com.benchmark.model.progress` (Progress, BashProgress, HookProgress)
- Convert mutable classes to records where possible
- Add consistent JavaDoc

**Files to Reorganize:**
- Move files into subpackages
- Update imports throughout codebase

---

### 3.3 Improve Configuration Structure ⚠️

**Priority:** LOW  
**Estimated Effort:** 1 day  
**Risk:** LOW

**Problem:**
- Configuration split across 6 classes
- Circular dependencies in setters
- No hierarchical validation

**Solution:**
```yaml
# Better YAML structure
benchmark:
  path: ../polyglot-benchmark
  parallelism: 4
  
docker:
  image: claude-benchmark/runner:latest
  memory: 2g
  timeout: 300
  environment:
    - ANTHROPIC_MODEL: sonnet

output:
  results_dir: ../benchmark-results
  log_level: INFO

agents:
  claude:
    cli_path: /usr/local/bin/claude
    model: sonnet
```

**Files to Modify:**
- All config classes - Simplify structure
- ConfigLoader - Add hierarchical validation

---

### 3.4 Add Comprehensive Testing ⚠️⚠️⚠️

**Priority:** HIGH  
**Estimated Effort:** 1 week  
**Risk:** LOW (adding tests, not changing code)

**Current State:**
- Only 2 test files: `LogParserTest.java`, `MessageProcessorTest.java`
- No integration tests
- No tests for core business logic

**Test Coverage Needed:**

| Component | Test Type | Priority |
|-----------|-----------|----------|
| ResultPersister | Unit tests | HIGH |
| AgentFactory | Unit tests | HIGH |
| Config validation | Unit tests | HIGH |
| ExerciseRunner | Integration tests | MEDIUM |
| DockerClient | Mock tests | MEDIUM |
| BenchmarkService | Integration tests | MEDIUM |
| LanguageHandlers | Unit tests (after 2.1) | HIGH |

**Files to Create:**
- `src/test/java/com/benchmark/persistence/ResultPersisterTest.java`
- `src/test/java/com/benchmark/agent/AgentFactoryTest.java`
- `src/test/java/com/benchmark/config/ConfigValidationTest.java`
- `src/test/java/com/benchmark/exercise/ExerciseRunnerIntegrationTest.java`
- `src/test/java/com/benchmark/docker/DockerClientMockTest.java`

---

### 3.5 Add Documentation ⚠️

**Priority:** MEDIUM  
**Estimated Effort:** 2 days  
**Risk:** LOW

**Documentation Needed:**

1. **Architecture Overview** (`docs/ARCHITECTURE.md`)
   - Component diagram
   - Data flow
   - Execution lifecycle

2. **API Documentation** (`docs/API.md`)
   - REST endpoints
   - Request/Response formats
   - Error codes

3. **Configuration Reference** (`docs/CONFIGURATION.md`)
   - All config options
   - Examples
   - Validation rules

4. **Developer Guide** (`docs/DEVELOPER.md`)
   - Building the project
   - Running tests
   - Adding new languages
   - Adding new agents

5. **Result Format** (`docs/RESULT_FORMAT.md`)
   - JSON structure
   - File naming conventions
   - Trace format

**Files to Create:**
- `docs/ARCHITECTURE.md`
- `docs/API.md`
- `docs/CONFIGURATION.md`
- `docs/DEVELOPER.md`
- `docs/RESULT_FORMAT.md`

---

## 📋 Quick Reference: All Action Items

### Immediate (Phase 2 - Next 2 weeks)
- [ ] 2.1 Refactor ReferenceAgent with Strategy Pattern (3-4 days)
- [ ] 2.2 Split BenchmarkController (1 day)
- [ ] 2.3 Extract CLI Entry Point (1 day)
- [ ] 2.4 Improve Error Handling (1 day)
- [ ] 2.5 Split BenchmarkService (2 days)
- [ ] 2.6 Fix Build Warnings (2 hours)

### Future (Phase 3 - Next month)
- [ ] 3.1 Refactor BenchmarkResultAnalyzer (2 days)
- [ ] 3.2 Consolidate Model Classes (2 days)
- [ ] 3.3 Improve Configuration Structure (1 day)
- [ ] 3.4 Add Comprehensive Testing (1 week)
- [ ] 3.5 Add Documentation (2 days)

---

## 🎯 Success Criteria

### Phase 2 Completion:
- ✅ ReferenceAgent < 200 lines (from 600)
- ✅ Each controller < 100 lines
- ✅ CLI logic isolated in dedicated class
- ✅ No empty catch blocks
- ✅ All build warnings resolved

### Phase 3 Completion:
- ✅ Test coverage > 70%
- ✅ Complete API documentation
- ✅ Architecture diagram updated
- ✅ All models in logical packages

---

## 📊 Progress Tracking

| Phase | Status | Items | Completed | Remaining |
|-------|--------|-------|-----------|-----------|
| Phase 1 | ✅ DONE | 4 | 4 | 0 |
| Phase 2 | 🚧 IN PROGRESS | 6 | 0 | 6 |
| Phase 3 | ⏳ PLANNED | 5 | 0 | 5 |

**Total:** 15 items, 4 completed (27%), 11 remaining

---

Last Updated: 2026-02-28  
Next Review: After Phase 2 completion

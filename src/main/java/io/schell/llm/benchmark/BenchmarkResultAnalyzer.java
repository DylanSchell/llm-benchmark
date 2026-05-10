package io.schell.llm.benchmark;

import io.schell.llm.benchmark.model.claude.*;
import io.schell.llm.benchmark.model.pi.PiLogEntry;
import io.schell.llm.benchmark.model.pi.PiMessage;
import io.schell.llm.benchmark.model.pi.PiMessageMessage;
import io.schell.llm.benchmark.model.pi.PiUsage;
import com.fasterxml.jackson.core.JsonParser;
import com.fasterxml.jackson.core.JsonToken;
import com.fasterxml.jackson.core.StreamReadConstraints;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.json.JsonMapper;

import java.io.*;
import java.nio.file.*;
import java.nio.file.attribute.FileTime;
import java.util.*;
import java.util.regex.*;
import java.util.stream.Stream;

/**
 * Analyzes benchmark result JSON files and generates a Markdown report.
 *
 * The tool scans a directory tree for result JSON files and associated token usage logs,
 * aggregates statistics per benchmark and per exercise, and writes a summary report to
 * {@code results.md}. It also copies raw trace logs into the user’s Claude projects folder
 * for further inspection.
 */
public class BenchmarkResultAnalyzer {
    List<SimpleResult> allResults = new ArrayList<>();
    Set<String> allExercises = new HashSet<>();
    Map<String, BenchmarkStats> statsByBenchmark = new LinkedHashMap<>();
    Map<String, List<SimpleResult>> resultsByExercise = new HashMap<>();
    Map<String, List<SimpleResult>> resultsByBenchmark = new LinkedHashMap<>();
    final ObjectMapper mapper;

    public BenchmarkResultAnalyzer() {
        mapper = JsonMapper.builder(new ObjectMapper().getFactory().setStreamReadConstraints(
                StreamReadConstraints.defaults().rebuild().maxStringLength(25000000).build())).build();
    }

    public static void main(String[] args) throws IOException {
        Path baseDir = Paths.get("../benchmark-results");
        BenchmarkResultAnalyzer benchmarkResultAnalyzer = new BenchmarkResultAnalyzer();
        benchmarkResultAnalyzer.loadBenchmarkResults(baseDir);
        benchmarkResultAnalyzer.archiveClaudeProjects(baseDir);
        benchmarkResultAnalyzer.generateReport();
    }

    private void processResultsDirectory(Path dir) {
        try {
            // Find result files in this directory (but not timestamped result files)
            List<PathTime> resultFiles = new ArrayList<>(Files.list(dir)
                    .filter(p -> p.toString().endsWith(".json"))
                    .filter(p-> p.getFileName().toString().startsWith("result_pi_")|| p.getFileName().toString().startsWith("result_claude") || p.getFileName().toString().startsWith("result_reference"))
                    .filter(p -> !Pattern.matches(".*result_claude_\\d{8}_\\d{6}\\.json", p.getFileName().toString())) // Exclude timestamped files
                    .map(p -> {
                        try {
                            return new PathTime(p, Files.getLastModifiedTime(p));
                        } catch (IOException e) {
                            throw new RuntimeException(e);
                        }
                    })
                    .sorted()
                    .toList());

            List<PathTime> traceFiles = new ArrayList<>(Files.list(dir)
                    .filter(p -> p.toString().endsWith("jsonl"))
                    .map(p -> {
                        try {
                            return new PathTime(p, Files.getLastModifiedTime(p));
                        } catch (IOException e) {
                            throw new RuntimeException(e);
                        }
                    })
                    .sorted()
                    .toList());

            if (!resultFiles.isEmpty()) {
                String benchmarkName = dir.getFileName().toString();
                BenchmarkStats stats = new BenchmarkStats(benchmarkName);

                for (PathTime pt : resultFiles) {
                    var resultFile = pt.path;
                    try {
                        SimpleResult simple = mapper.readValue(resultFile.toFile(), SimpleResult.class);
                        simple.model = benchmarkName;
                        allResults.add(simple);
                        allExercises.add(simple.exerciseName+"_"+simple.language);
                        resultsByBenchmark.computeIfAbsent(benchmarkName, k -> new ArrayList<>())
                                .add(simple);
                        resultsByExercise.computeIfAbsent(simple.exerciseName+"_"+simple.language, k -> new ArrayList<>())
                                .add(simple);
                        stats.totalResults++;
                        if (simple.success) {
                            stats.successResults++;
                        } else {
                            stats.failedResults++;
                        }
                        stats.totalDuration += simple.duration;
                        stats.exitCode = simple.exitCode;
                        if (simple.exitCode != 0) {
                            if (simple.success) {
                                // this was not correctly marked as a success, flip the outcome
                                stats.successResults--;
                                stats.failedResults++;
                            }
                        }
                        FileTime resultTime = Files.getLastModifiedTime(resultFile);
                        while (!traceFiles.isEmpty() && resultTime.compareTo(traceFiles.get(0).fileTime) > 0) {
                            PathTime pt2 = traceFiles.remove(0);
                            Path tracePath = pt2.path;
                            TokenUsage usage = simple.model.startsWith("pi") ? calculatePiTokens(tracePath) : calculateClaudeTokens(tracePath);
                            stats.inputTokens += usage.input_tokens();
                            stats.outputTokens += usage.output_tokens();
                            stats.cachedInputTokens += usage.cached_input_tokens();
                            stats.uncachedInputTokens += usage.uncached_input_tokens();
                            simple.inputTokens += usage.input_tokens();
                            simple.outputTokens += usage.output_tokens();
                            simple.cachedInputTokens += usage.cached_input_tokens();
                            simple.uncachedInputTokens += usage.uncached_input_tokens();
                        }
                    } catch (Exception e) {
                        System.err.println("Warning: Could not read " + resultFile + ": " + e.getMessage());
                    }
                }
                if (stats.totalResults > 0) {
                    statsByBenchmark.put(benchmarkName, stats);
                }
            }
        } catch (IOException e) {
            System.err.println("Error scanning directory " + dir + ": " + e.getMessage());
        }
    }

    private static class PathTime implements Comparable<PathTime> {
        private final Path path;
        private final FileTime fileTime;

        public PathTime(Path path, FileTime fileTime) {
            this.path = path;
            this.fileTime = fileTime;
        }

        @Override
        public int compareTo(PathTime o) {
            return fileTime.compareTo(o.fileTime);
        }
    }

    public void loadBenchmarkResults(Path baseDir) throws IOException {
        // Find all directories that contain result_claude*.json files
        try (Stream<Path> walk = Files.walk(baseDir)) {
            walk.filter(Files::isDirectory)
                    .forEach(this::processResultsDirectory);
        }

    }

    record TokenUsage(long input_tokens, long output_tokens, long cached_input_tokens, long uncached_input_tokens) {
    }


    private TokenUsage calculatePiTokens(Path tracePath) throws IOException {
        long inputTokens = 0;
        long outputTokens = 0;
        long cachedInputTokens = 0;
        long uncachedInputTokens = 0;
        // Use a BufferedReader to avoid loading the whole file
        try (BufferedReader br = Files.newBufferedReader(tracePath);
             // Jackson's streaming parser works directly on the reader
             JsonParser parser = mapper.getFactory().createParser(br)) {

            // Walk through each line (JSON object) in the file
            while (!parser.isClosed()) {
                JsonToken token = parser.nextToken();
                if (token == JsonToken.START_OBJECT) {
                    // Parse just the current object with polymorphic type resolution
                    PiLogEntry entry = mapper.readValue(parser, PiLogEntry.class);
                    if (entry instanceof PiMessage) {
                        PiMessage message = (PiMessage) entry;
                        if ( message.message instanceof PiMessageMessage ) {
                            PiMessageMessage messageMessage = (PiMessageMessage) message.message;
                            if ( messageMessage.usage instanceof PiUsage ) {
                                PiUsage usage = (PiUsage) messageMessage.usage;
                                inputTokens += usage.input;
                                outputTokens += usage.output;
                            }
                        }
                    }
                }
            }
        }
        TokenUsage usage = new TokenUsage(inputTokens,outputTokens,cachedInputTokens,uncachedInputTokens);
        return usage;
    }

    private TokenUsage calculateClaudeTokens(Path tracePath) throws IOException {
        long inputTokens = 0;
        long outputTokens = 0;
        long cachedInputTokens = 0;
        long uncachedInputTokens = 0;
        long previousInputTokens = 0;

        // Use a BufferedReader to avoid loading the whole file
        try (BufferedReader br = Files.newBufferedReader(tracePath);
             // Jackson's streaming parser works directly on the reader
             JsonParser parser = mapper.getFactory().createParser(br)) {

            // Walk through each line (JSON object) in the file
            while (!parser.isClosed()) {
                JsonToken token = parser.nextToken();
                if (token == JsonToken.START_OBJECT) {
                    // Parse just the current object with polymorphic type resolution
                    LogEntry entry = mapper.readValue(parser, LogEntry.class);
                    Message message = null;
                    if (entry instanceof AssistantEntry assistantEntry) {
                        message = assistantEntry.getMessage();
                    } else if (entry instanceof UserEntry userEntry) {
                        message = userEntry.getMessage();
                    }

                    if (message != null) {
                        Usage usage = message.getUsage();
                        if (usage != null) {
                            inputTokens += usage.getInputTokens();
                            outputTokens += usage.getOutputTokens();

                            // Calculate new input tokens (difference from previous)
                            long newInputTokens = usage.getInputTokens() - previousInputTokens;
                            if (newInputTokens > 0) {
                                uncachedInputTokens += newInputTokens;
                                cachedInputTokens += usage.getInputTokens() - newInputTokens;
                            } else {
                                uncachedInputTokens += usage.getInputTokens();
                            }
                            previousInputTokens = usage.getInputTokens();
                        }
                    }
                }
            }
        }
        return new TokenUsage(inputTokens, outputTokens, cachedInputTokens, uncachedInputTokens);
    }

    public void generateReport() throws IOException {
        // Sort by completion percentage (desc), then by total duration (asc) for ties
        List<BenchmarkStats> sortedStats = new ArrayList<>(statsByBenchmark.values());
        sortedStats.sort(this::sortByCountAndPercentage);


        // Generate markdown output
        StringBuilder markdown = new StringBuilder();
        markdown.append("# Benchmark Results Summary\n\n");
        markdown.append("| Benchmark | Total Results | Success | Failed | Completion % | Total Duration | Tokens |\n");
        markdown.append("|-----------|---------------|---------|--------|---------------|----------------|--------|\n");

        dumpSortedStats(sortedStats, markdown);

        markdown.append("\n");
        markdown.append("# Success rates per exercise\n\n");
        markdown.append("| Exercise | Total Results | Success | Failed | Completion % | Total Duration | Tokens |\n");
        markdown.append("|----------|---------------|---------|--------|---------------|----------------|--------|\n");
        List<BenchmarkStats> exerciseStats = new ArrayList<>();
        for (String exerciseName : allExercises) {
            BenchmarkStats stats = new BenchmarkStats(exerciseName);
            exerciseStats.add(stats);
            for (SimpleResult simpleResult : allResults) {
                if (simpleResult.exerciseName.equals(exerciseName)) {
                    stats.totalResults++;
                    if (simpleResult.success) {
                        stats.successResults++;
                    } else {
                        stats.failedResults++;
                    }
                    stats.totalDuration += simpleResult.duration;
                    stats.inputTokens += simpleResult.inputTokens;
                    stats.outputTokens += simpleResult.outputTokens;
                    stats.cachedInputTokens += simpleResult.cachedInputTokens;
                    stats.uncachedInputTokens += simpleResult.uncachedInputTokens;
                }
            }
        }

        exerciseStats.sort(this::sortByPercentage);
        dumpSortedStats(exerciseStats, markdown);

        // Breakdown of individual benchmark runs per model confgururation
        resultsByBenchmark.forEach((benchmarkName, results) -> {
            markdown.append("\n");
            markdown.append(String.format("# %s\n\n", benchmarkName.replace(".","_").replace(':','-')));
            markdown.append("| Exercise | Success | Duration | Tokens |\n");
            markdown.append("|----------|---------|----------|--------|\n");
            for (SimpleResult simpleResult : results) {
                markdown.append(String.format("| [%s](#%s) | %s | %s | %s |\n",
                        simpleResult.exerciseName.replace(".","_")+"_"+simpleResult.language,
                        simpleResult.exerciseName.replace(".","_")+"_"+simpleResult.language,
                        simpleResult.success ? "✅" : simpleResult.duration >= 7199 ? "⏰" : "❌",
                        formatDuration(simpleResult.duration),
                        formatTokens(simpleResult.uncachedInputTokens, simpleResult.cachedInputTokens, simpleResult.outputTokens)));
            }
            markdown.append("\n");
        });

        // Breakdown per exercise how long each model took, and if it succeeded
        resultsByExercise.forEach((exercise, results) -> {
            markdown.append("\n");
            markdown.append(String.format("# %s\n\n", exercise.replace(".","_")));
            markdown.append("| Model | Success | Duration | Tokens |\n");
            markdown.append("|-------|---------|----------|--------|\n");
            results.sort(Comparator.comparingDouble(o -> o.duration));
            for (SimpleResult simpleResult : results) {
                markdown.append(String.format("| [%s](#%s) | %s | %s | %s |\n",
                        simpleResult.model.replace(".","_"),
                        simpleResult.model.replace(".","_"),
                        simpleResult.success ? "✅" : "❌",
                        formatDuration(simpleResult.duration),
                        formatTokens(simpleResult.uncachedInputTokens, simpleResult.cachedInputTokens, simpleResult.outputTokens)));
            }
            markdown.append("\n");
        });

        markdown.append("\n*Generated by BenchmarkResultAnalyzer*\n");

        // Write to results.md
        Files.writeString(Paths.get("results.md"), markdown.toString());
        System.out.println("Results written to results.md");
        System.out.println("\n" + markdown);
    }

    private void dumpSortedStats(List<BenchmarkStats> sortedStats, StringBuilder markdown) {
        for (BenchmarkStats stats : sortedStats) {
            double completionPercent = (stats.successResults * 100.0) / stats.totalResults;
            String durationStr = formatDuration(stats.totalDuration);
            markdown.append(String.format("| [%s](#%s) | %d | %d | %d | %.1f%% | %s | %s |\n",
                    stats.benchmarkName.replace(".","_").replace(':','-'),
                    stats.benchmarkName.replace(".","_").replace(':','-'),
                    stats.totalResults,
                    stats.successResults,
                    stats.failedResults,
                    completionPercent,
                    durationStr,
                    formatTokens(stats.uncachedInputTokens, stats.cachedInputTokens, stats.outputTokens)));
        }
    }

    private int sortByPercentage(BenchmarkStats statsA, BenchmarkStats statsB) {
        double compA = (statsA.successResults * 100.0) / statsA.totalResults;
        double compB = (statsB.successResults * 100.0) / statsB.totalResults;
        int cmp = Double.compare(compB, compA); // descending
        if (cmp != 0) return cmp;
        return Double.compare(statsA.totalDuration, statsB.totalDuration); // ascending
    }

    private int sortByCountAndPercentage(BenchmarkStats statsA, BenchmarkStats statsB) {
        int cmp = Long.compare(statsB.totalResults, statsA.totalResults);
        if (cmp != 0) return cmp;
        double compA = (statsA.successResults * 100.0) / statsA.totalResults;
        double compB = (statsB.successResults * 100.0) / statsB.totalResults;
        cmp = Double.compare(compB, compA); // descending
        if (cmp != 0) return cmp;
        return Double.compare(statsA.totalDuration, statsB.totalDuration); // ascending
    }


    private void archiveClaudeProjects(Path baseDir) throws IOException {
        // copy all logs to .claude projects, so ccusage and the like can read it
        Path targetDir = Paths.get("/Users/dylan/.claude/projects/benchmark");
        try (Stream<Path> stream = Files.walk(baseDir)) {
            stream.filter(path -> path.toString().endsWith(".jsonl"))
                    .forEach(source -> {
                        Path target = targetDir.resolve(source.getFileName());
                        try {
                            if (!Files.exists(target)) {
                                Files.copy(source, target);
                            }
                        } catch (IOException e) {
                            System.err.println("Error copying " + source.getFileName() + ": " + e.getMessage());
                        }
                    });
        }
    }

    private static String formatTokens(long uncachedInputTokens, long cachedInputTokens, long outputTokens) {
        return String.format("%s / %s / %s", formatNumber(uncachedInputTokens), formatNumber(cachedInputTokens), formatNumber(outputTokens));
    }

    private static String formatNumber(long num) {
        if (num >= 1_000_000_000) {
            return String.format("%.1fG", num / 1_000_000_000.0);
        } else if (num >= 1_000_000) {
            return String.format("%.1fM", num / 1_000_000.0);
        } else if (num >= 1_000) {
            return String.format("%.1fK", num / 1_000.0);
        } else {
            return String.valueOf(num);
        }
    }

    private static String formatDuration(double totalSeconds) {
        if (totalSeconds == 0) return "0s";

        int days = (int) (totalSeconds / 86400);
        int hours = (int) ((totalSeconds % 86400) / 3600);
        int minutes = (int) ((totalSeconds % 3600) / 60);
        int seconds = (int) (totalSeconds % 60);

        StringBuilder sb = new StringBuilder();
        if (days > 0) sb.append(days).append("d ");
        if (hours > 0) sb.append(hours).append("h ");
        if (minutes > 0) sb.append(minutes).append("m ");
        sb.append(seconds).append("s");

        return sb.toString().trim();
    }

    private static class BenchmarkStats {
        String benchmarkName;
        int totalResults = 0;
        int successResults = 0;
        int failedResults = 0;
        double totalDuration = 0;
        int exitCode = 0;
        long inputTokens = 0;
        long outputTokens = 0;
        long cachedInputTokens = 0;
        long uncachedInputTokens = 0;

        BenchmarkStats(String name) {
            this.benchmarkName = name;
        }
    }

    private static class SimpleResult {
        public String model;
        public String language;
        public String exerciseName;
        public double duration;
        public String output;
        public boolean success;
        public int exitCode;
        public long inputTokens;
        public long outputTokens;
        public long cachedInputTokens;
        public long uncachedInputTokens;
        public String startTime;
        public String endTime;
        public String errorMessage;
        public String trace;
        public int attempts;
    }
}

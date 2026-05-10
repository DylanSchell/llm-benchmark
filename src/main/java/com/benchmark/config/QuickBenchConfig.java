package com.benchmark.config;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Configuration for "Quick bench" mode — a curated list of exercises known to complete
 * in under 60 seconds per the pi-qwen36-35b-a3b-q4-no-thinking benchmark run.
 *
 * Each entry is a map from language to list of exercise names.
 */
public class QuickBenchConfig {

    private static final Map<String, List<String>> QUICK_EXERCISES = new LinkedHashMap<>();

    static {
        // C++ — 23 exercises under 60s
        List<String> cppExercises = new ArrayList<>();
        cppExercises.add("all-your-base");
        cppExercises.add("allergies");
        cppExercises.add("bank-account");
        cppExercises.add("binary-search-tree");
        cppExercises.add("circular-buffer");
        cppExercises.add("clock");
        cppExercises.add("crypto-square");
        cppExercises.add("diamond");
        cppExercises.add("dnd-character");
        cppExercises.add("gigasecond");
        cppExercises.add("grade-school");
        cppExercises.add("kindergarten-garden");
        cppExercises.add("knapsack");
        cppExercises.add("linked-list");
        cppExercises.add("parallel-letter-frequency");
        cppExercises.add("perfect-numbers");
        cppExercises.add("phone-number");
        cppExercises.add("queen-attack");
        cppExercises.add("robot-name");
        cppExercises.add("space-age");
        cppExercises.add("spiral-matrix");
        cppExercises.add("sublist");
        cppExercises.add("yacht");
        QUICK_EXERCISES.put("cpp", cppExercises);

        // Go — 25 exercises under 60s
        List<String> goExercises = new ArrayList<>();
        goExercises.add("beer-song");
        goExercises.add("book-store");
        goExercises.add("bottle-song");
        goExercises.add("crypto-square");
        goExercises.add("dnd-character");
        goExercises.add("dominoes");
        goExercises.add("error-handling");
        goExercises.add("food-chain");
        goExercises.add("hexadecimal");
        goExercises.add("octal");
        goExercises.add("paasio");
        goExercises.add("palindrome-products");
        goExercises.add("pig-latin");
        goExercises.add("protein-translation");
        goExercises.add("say");
        goExercises.add("simple-linked-list");
        goExercises.add("sublist");
        goExercises.add("transpose");
        goExercises.add("tree-building");
        goExercises.add("trinary");
        goExercises.add("two-bucket");
        goExercises.add("variable-length-quantity");
        goExercises.add("word-search");
        goExercises.add("wordy");
        QUICK_EXERCISES.put("go", goExercises);

        // Java — 28 exercises under 60s
        List<String> javaExercises = new ArrayList<>();
        javaExercises.add("affine-cipher");
        javaExercises.add("all-your-base");
        javaExercises.add("bank-account");
        javaExercises.add("book-store");
        javaExercises.add("bottle-song");
        javaExercises.add("change");
        javaExercises.add("circular-buffer");
        javaExercises.add("custom-set");
        javaExercises.add("dominoes");
        javaExercises.add("house");
        javaExercises.add("kindergarten-garden");
        javaExercises.add("ocr-numbers");
        javaExercises.add("palindrome-products");
        javaExercises.add("phone-number");
        javaExercises.add("pig-latin");
        javaExercises.add("protein-translation");
        javaExercises.add("pythagorean-triplet");
        javaExercises.add("queen-attack");
        javaExercises.add("resistor-color-trio");
        javaExercises.add("satellite");
        javaExercises.add("series");
        javaExercises.add("simple-linked-list");
        javaExercises.add("state-of-tic-tac-toe");
        javaExercises.add("transpose");
        javaExercises.add("tree-building");
        javaExercises.add("twelve-days");
        javaExercises.add("two-bucket");
        javaExercises.add("word-search");
        QUICK_EXERCISES.put("java", javaExercises);

        // JavaScript — 36 exercises under 60s
        List<String> jsExercises = new ArrayList<>();
        jsExercises.add("affine-cipher");
        jsExercises.add("alphametics");
        jsExercises.add("beer-song");
        jsExercises.add("binary");
        jsExercises.add("book-store");
        jsExercises.add("bottle-song");
        jsExercises.add("connect");
        jsExercises.add("food-chain");
        jsExercises.add("go-counting");
        jsExercises.add("grade-school");
        jsExercises.add("grep");
        jsExercises.add("killer-sudoku-helper");
        jsExercises.add("list-ops");
        jsExercises.add("meetup");
        jsExercises.add("ocr-numbers");
        jsExercises.add("palindrome-products");
        jsExercises.add("phone-number");
        jsExercises.add("pig-latin");
        jsExercises.add("promises");
        jsExercises.add("queen-attack");
        jsExercises.add("rational-numbers");
        jsExercises.add("rectangles");
        jsExercises.add("resistor-color-trio");
        jsExercises.add("robot-name");
        jsExercises.add("say");
        jsExercises.add("scale-generator");
        jsExercises.add("simple-linked-list");
        jsExercises.add("space-age");
        jsExercises.add("state-of-tic-tac-toe");
        jsExercises.add("sum-of-multiples");
        jsExercises.add("tournament");
        jsExercises.add("transpose");
        jsExercises.add("triangle");
        jsExercises.add("twelve-days");
        jsExercises.add("two-bucket");
        jsExercises.add("variable-length-quantity");
        jsExercises.add("word-search");
        jsExercises.add("wordy");
        jsExercises.add("zipper");
        QUICK_EXERCISES.put("javascript", jsExercises);

        // Python — 23 exercises under 60s
        List<String> pythonExercises = new ArrayList<>();
        pythonExercises.add("affine-cipher");
        pythonExercises.add("beer-song");
        pythonExercises.add("book-store");
        pythonExercises.add("bottle-song");
        pythonExercises.add("dominoes");
        pythonExercises.add("food-chain");
        pythonExercises.add("go-counting");
        pythonExercises.add("grade-school");
        pythonExercises.add("grep");
        pythonExercises.add("list-ops");
        pythonExercises.add("phone-number");
        pythonExercises.add("pig-latin");
        pythonExercises.add("proverb");
        pythonExercises.add("rest-api");
        pythonExercises.add("robot-name");
        pythonExercises.add("simple-linked-list");
        pythonExercises.add("transpose");
        pythonExercises.add("tree-building");
        pythonExercises.add("two-bucket");
        pythonExercises.add("variable-length-quantity");
        pythonExercises.add("wordy");
        pythonExercises.add("zebra-puzzle");
        pythonExercises.add("zipper");
        QUICK_EXERCISES.put("python", pythonExercises);

        // Rust — 20 exercises under 60s
        List<String> rustExercises = new ArrayList<>();
        rustExercises.add("accumulate");
        rustExercises.add("acronym");
        rustExercises.add("alphametics");
        rustExercises.add("book-store");
        rustExercises.add("dot-dsl");
        rustExercises.add("gigasecond");
        rustExercises.add("grade-school");
        rustExercises.add("grep");
        rustExercises.add("luhn-from");
        rustExercises.add("macros");
        rustExercises.add("nucleotide-codons");
        rustExercises.add("parallel-letter-frequency");
        rustExercises.add("pig-latin");
        rustExercises.add("robot-name");
        rustExercises.add("say");
        rustExercises.add("two-bucket");
        rustExercises.add("variable-length-quantity");
        rustExercises.add("word-count");
        QUICK_EXERCISES.put("rust", rustExercises);
    }

    /**
     * Returns the list of quick-bench exercises for a given language.
     *
     * @param language The programming language
     * @return List of exercise names that complete in under 60 seconds, or empty list if not available
     */
    public static List<String> getExercisesForLanguage(String language) {
        return QUICK_EXERCISES.getOrDefault(language, List.of());
    }

    /**
     * Returns all languages that have quick-bench exercises defined.
     */
    public static List<String> getAvailableLanguages() {
        return new ArrayList<>(QUICK_EXERCISES.keySet());
    }

    /**
     * Returns the total number of quick-bench exercise slots across all languages.
     */
    public static int getTotalExerciseCount() {
        return QUICK_EXERCISES.values().stream().mapToInt(List::size).sum();
    }
}

package io.schell.llm.benchmark.util;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.*;
import java.nio.file.attribute.BasicFileAttributes;
import java.util.Collection;

/**
 * Utility class for common file operations used across language handlers.
 */
public class FileUtils {
    private static final Logger logger = LoggerFactory.getLogger(FileUtils.class);

    private FileUtils() {
        // Prevent instantiation
    }

    /**
     * Copies files from a collection of source paths to a destination directory.
     * Optionally filters by file extension.
     *
     * @param sourcePaths Collection of source file paths
     * @param destDir     Destination directory
     * @param fileFilter  File extension filter (e.g., ".java"), or null for no filter
     * @throws IOException if file operations fail
     */
    public static void copyFilesFromPaths(Iterable<Path> sourcePaths, Path destDir, String fileFilter) throws IOException {
        for (Path sourcePath : sourcePaths) {
            if (sourcePath == null || !Files.exists(sourcePath)) {
                continue;
            }

            if (fileFilter != null && !sourcePath.toString().endsWith(fileFilter)) {
                continue;
            }

            String fileName = sourcePath.getFileName().toString();
            Path destFile = destDir.resolve(fileName);
            Files.copy(sourcePath, destFile, StandardCopyOption.REPLACE_EXISTING);
            logger.info("Copied file: {}", fileName);
        }
    }

    /**
     * Copies all files from a source directory to a destination directory,
     * excluding paths that contain the given exclusion pattern.
     *
     * @param sourceDir      Source directory
     * @param destDir        Destination directory
     * @param exclusionPattern Pattern to exclude (e.g., ".meta/")
     * @throws IOException if file operations fail
     */
    public static void copyDirectoryExcluding(Path sourceDir, Path destDir, String exclusionPattern) throws IOException {
        try (java.util.stream.Stream<Path> paths = Files.walk(sourceDir)) {
            paths.forEach(sourcePath -> {
                try {
                    Path relativePath = sourceDir.relativize(sourcePath);

                    // Skip excluded paths
                    if (exclusionPattern != null && relativePath.toString().contains(exclusionPattern)) {
                        return;
                    }

                    Path destPath = destDir.resolve(relativePath);

                    if (Files.isDirectory(sourcePath)) {
                        Files.createDirectories(destPath);
                    } else {
                        Files.copy(sourcePath, destPath, StandardCopyOption.REPLACE_EXISTING);
                    }
                } catch (IOException e) {
                    logger.error("Failed to copy file {}: {}", sourcePath, e.getMessage());
                }
            });
        }
    }

    /**
     * Recursively replaces text patterns in files with the given extension.
     * Used for removing test annotations like @Disabled or #[ignore].
     *
     * @param directory       Directory to search
     * @param fileExtension   File extension to match (e.g., ".java")
     * @param pattern         Regex pattern to replace
     * @param replacement     Replacement string
     * @return Number of files that failed to process
     * @throws IOException if directory traversal fails
     */
    public static int replaceInFilesRecursive(Path directory, String fileExtension, 
                                               String pattern, String replacement) throws IOException {
        if (!Files.exists(directory)) {
            return 0;
        }

        final int[] errorCount = {0};
        Files.walkFileTree(directory, new FileVisitor<Path>() {
            @Override
            public FileVisitResult preVisitDirectory(Path dir, BasicFileAttributes attrs) {
                return FileVisitResult.CONTINUE;
            }

            @Override
            public FileVisitResult visitFile(Path file, BasicFileAttributes attrs) {
                if (file.toString().endsWith(fileExtension)) {
                    try {
                        String content = Files.readString(file);
                        String updated = content.replaceAll(pattern, replacement);
                        Files.writeString(file, updated);
                    } catch (IOException e) {
                        errorCount[0]++;
                        logger.error("Failed to patch {}: {}", file.getFileName(), e.getMessage());
                    }
                }
                return FileVisitResult.CONTINUE;
            }

            @Override
            public FileVisitResult visitFileFailed(Path file, IOException exc) {
                return FileVisitResult.CONTINUE;
            }

            @Override
            public FileVisitResult postVisitDirectory(Path dir, IOException exc) {
                return FileVisitResult.CONTINUE;
            }
        });

        return errorCount[0];
    }
}

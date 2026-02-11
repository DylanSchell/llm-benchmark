package com.benchmark.exercise;

import com.fasterxml.jackson.annotation.JsonProperty;

import java.util.List;

/**
 * Metadata for an exercise parsed from .meta/config.json
 */
public class ExerciseMetadata {

    @JsonProperty("authors")
    private List<String> authors;

    @JsonProperty("contributors")
    private List<String> contributors;

    @JsonProperty("files")
    private Files files;

    @JsonProperty("blurb")
    private String blurb;

    @JsonProperty("source")
    private String source;

    @JsonProperty("source_url")
    private String source_url;

    public List<String> getAuthors() {
        return authors;
    }

    public void setAuthors(List<String> authors) {
        this.authors = authors;
    }

    public List<String> getContributors() {
        return contributors;
    }

    public void setContributors(List<String> contributors) {
        this.contributors = contributors;
    }

    public Files getFiles() {
        return files;
    }

    public void setFiles(Files files) {
        this.files = files;
    }

    public String getBlurb() {
        return blurb;
    }

    public void setBlurb(String blurb) {
        this.blurb = blurb;
    }

    public String getSource() {
        return source;
    }

    public void setSource(String source) {
        this.source = source;
    }

    public String getSource_url() {
        return source_url;
    }

    public void setSource_url(String source_url) {
        this.source_url = source_url;
    }

    /**
     * File categories from config.json
     */
    public static class Files {
        @JsonProperty("solution")
        private List<String> solution;

        @JsonProperty("test")
        private List<String> test;

        @JsonProperty("example")
        private List<String> example;

        @JsonProperty("editor")
        private List<String> editor;

        @JsonProperty("invalidator")
        private List<String> invalidator;

        public List<String> getSolution() {
            return solution;
        }

        public void setSolution(List<String> solution) {
            this.solution = solution;
        }

        public List<String> getTest() {
            return test;
        }

        public void setTest(List<String> test) {
            this.test = test;
        }

        public List<String> getExample() {
            return example;
        }

        public void setExample(List<String> example) {
            this.example = example;
        }

        public List<String> getEditor() {
            return editor;
        }

        public void setEditor(List<String> editor) {
            this.editor = editor;
        }

        public List<String> getInvalidator() {
            return invalidator;
        }

        public void setInvalidator(List<String> invalidator) {
            this.invalidator = invalidator;
        }
    }
}

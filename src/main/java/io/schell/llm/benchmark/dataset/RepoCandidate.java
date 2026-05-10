package io.schell.llm.benchmark.dataset;

/**
 * Represents a GitHub repository evaluated for SWE benchmark suitability.
 */
public record RepoCandidate(
    String owner,
    String name,
    String fullName,
    String url,
    String description,
    int stars,
    int forks,
    int openIssues,
    int contributors,
    String language,
    String license,
    boolean hasReadme,
    boolean isLibrary,
    int recentMergedPrs,  // PRs merged in last 90 days
    double score,
    String[] topBranches
) {
    public static Builder builder() {
        return new Builder();
    }

    public boolean isSuitable() {
        return score >= MIN_SCORE && isLibrary && "Java".equalsIgnoreCase(language);
    }

    public boolean hasGoodPRActivity() {
        return recentMergedPrs > 5;
    }

    private static final double MIN_SCORE = 30.0;

    public static class Builder {
        private String owner;
        private String name;
        private String fullName;
        private String url;
        private String description;
        private int stars;
        private int forks;
        private int openIssues;
        private int contributors;
        private String language;
        private String license;
        private boolean hasReadme;
        private boolean isLibrary;
        private int recentMergedPrs;
        private double score;
        private String[] topBranches;

        public Builder owner(String owner) { this.owner = owner; return this; }
        public Builder name(String name) { this.name = name; return this; }
        public Builder fullName(String fullName) { this.fullName = fullName; return this; }
        public Builder url(String url) { this.url = url; return this; }
        public Builder description(String description) { this.description = description; return this; }
        public Builder stars(int stars) { this.stars = stars; return this; }
        public Builder forks(int forks) { this.forks = forks; return this; }
        public Builder openIssues(int openIssues) { this.openIssues = openIssues; return this; }
        public Builder contributors(int contributors) { this.contributors = contributors; return this; }
        public Builder language(String language) { this.language = language; return this; }
        public Builder license(String license) { this.license = license; return this; }
        public Builder hasReadme(boolean hasReadme) { this.hasReadme = hasReadme; return this; }
        public Builder isLibrary(boolean isLibrary) { this.isLibrary = isLibrary; return this; }
        public Builder recentMergedPrs(int recentMergedPrs) { this.recentMergedPrs = recentMergedPrs; return this; }
        public Builder score(double score) { this.score = score; return this; }
        public Builder topBranches(String[] topBranches) { this.topBranches = topBranches; return this; }

        public RepoCandidate build() {
            return new RepoCandidate(owner, name, fullName, url, description, stars, forks,
                    openIssues, contributors, language, license, hasReadme, isLibrary,
                    recentMergedPrs, score, topBranches);
        }
    }
}

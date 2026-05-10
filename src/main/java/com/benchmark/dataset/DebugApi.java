package com.benchmark.dataset;

import java.util.List;

public class DebugApi {
    public static void main(String[] args) throws Exception {
        String token = System.getenv("GITHUB_TOKEN");
        if (token == null) {
            System.err.println("No token");
            return;
        }
        GithubApi api = new GithubApi(token);
        List<GithubApi.GithubPr> prs = api.searchPrs("language:java type:pr is:merged fix bug", 1);
        for (var pr : prs) {
            System.out.println("URL: " + pr.repositoryUrl());
            System.out.println("Owner: " + pr.getOwner());
            System.out.println("Repo: " + pr.getRepo());
            String[] parts = pr.repositoryUrl().split("/");
            System.out.println("Parts count: " + parts.length);
            for (int i = 0; i < parts.length; i++) {
                System.out.println("  [" + i + "] = " + parts[i]);
            }
        }
    }
}

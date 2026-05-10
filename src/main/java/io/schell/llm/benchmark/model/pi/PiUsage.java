package io.schell.llm.benchmark.model.pi;

public class PiUsage {
    public long input;
    public long output;
    public long cacheRead;
    public long cacheWrite;
    public long totalTokens;
    public PiCost cost;
}

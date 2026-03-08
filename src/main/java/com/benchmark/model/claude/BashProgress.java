package com.benchmark.model.claude;

public class BashProgress extends ProgressData {
    private String output;
    private String fullOutput;
    private int elapsedTimeSeconds;
    private int totalLines;
    private int totalBytes;
    private String taskId;

    public String getOutput() {
        return output;
    }

    public void setOutput(String output) {
        this.output = output;
    }


    public String getFullOutput() {
        return fullOutput;
    }

    public void setFullOutput(String fullOutput) {
        this.fullOutput = fullOutput;
    }


    public int getElapsedTimeSeconds() {
        return elapsedTimeSeconds;
    }

    public void setElapsedTimeSeconds(int elapsedTimeSeconds) {
        this.elapsedTimeSeconds = elapsedTimeSeconds;
    }

    public int getTotalLines() {
        return totalLines;
    }

    public void setTotalLines(int totalLines) {
        this.totalLines = totalLines;
    }

    public int getTotalBytes() {
        return totalBytes;
    }

    public void setTotalBytes(int totalBytes) {
        this.totalBytes = totalBytes;
    }

    public String getTaskId() {
        return taskId;
    }

    public void setTaskId(String taskId) {
        this.taskId = taskId;
    }
}

package com.benchmark.model;

public class HookProgress extends ProgressData {
    private String hookEvent;
    private String hookName;
        private String command;

    public String getHookEvent() {
        return hookEvent;
    }

    public void setHookEvent(String hookEvent) {
        this.hookEvent = hookEvent;
    }

    public String getHookName() {
        return hookName;
    }

    public void setHookName(String hookName) {
        this.hookName = hookName;
    }

    public String getCommand() {
        return command;
    }

    public void setCommand(String command) {
        this.command = command;
    }

}

package com.benchmark.web.config;

import com.benchmark.BenchmarkRunner;
import com.benchmark.config.Config;
import com.benchmark.config.ConfigLoader;
import com.benchmark.docker.DockerClient;
import com.benchmark.exercise.ExerciseRunner;
import com.benchmark.web.service.BenchmarkExecutor;
import com.benchmark.web.service.QueueProcessor;
import com.benchmark.web.service.SessionManager;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;
import org.springframework.scheduling.annotation.EnableAsync;
import org.springframework.web.servlet.config.annotation.ResourceHandlerRegistry;
import org.springframework.web.servlet.config.annotation.WebMvcConfigurer;

import java.nio.file.Paths;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.ThreadFactory;

/**
 * Web configuration for static resources and executors.
 */
@Configuration
@EnableAsync
public class WebConfig implements WebMvcConfigurer {

    @Override
    public void addResourceHandlers(ResourceHandlerRegistry registry) {
        registry.addResourceHandler("/static/**")
                .addResourceLocations("classpath:/static/");
    }

    /**
     * Config bean - loads from config.yaml.
     */
    @Bean
    public Config config() throws Exception {
        return ConfigLoader.load(Paths.get("config.yaml"));
    }

    /**
     * DockerClient bean - depends on Config.
     */
    @Bean
    public DockerClient dockerClient(Config config) {
        return new DockerClient(config.getDocker());
    }

    /**
     * BenchmarkRunner bean - depends on Config and DockerClient.
     * ExerciseRunner is created lazily to avoid circular dependency.
     */
    @Bean
    public BenchmarkRunner benchmarkRunner(Config config, DockerClient dockerClient) throws Exception {
        return new BenchmarkRunner(config, dockerClient);
    }

    /**
     * ExerciseRunner bean - depends on Config, DockerClient, and BenchmarkRunner.
     * Uses @Lazy to break circular dependency with BenchmarkRunner.
     */
    @Bean
    public ExerciseRunner exerciseRunner(Config config, DockerClient dockerClient, BenchmarkRunner benchmarkRunner) {
        return new ExerciseRunner(config, dockerClient, benchmarkRunner);
    }

    /**
     * Executor service with daemon threads for background tasks.
     * Spring will manage the lifecycle - shutting it down on application close.
     */
    @Bean
    public ExecutorService benchmarkExecutor() {
        return Executors.newCachedThreadPool(new DaemonThreadFactory());
    }

    /**
     * ResultService bean - depends on Config.
     */
    @Bean
    public com.benchmark.web.service.ResultService resultService(Config config) {
        return new com.benchmark.web.service.ResultService(config);
    }

    /**
     * SessionManager bean.
     */
    @Bean
    public SessionManager sessionManager() {
        return new SessionManager();
    }

    /**
     * BenchmarkExecutor bean - depends on BenchmarkRunner and DockerClient.
     */
    @Bean
    public BenchmarkExecutor benchmarkExecutor(BenchmarkRunner benchmarkRunner, DockerClient dockerClient) {
        return new BenchmarkExecutor(benchmarkRunner, dockerClient);
    }

    /**
     * QueueProcessor bean - depends on other services.
     */
    @Bean
    public QueueProcessor queueProcessor(SessionManager sessionManager, 
                                        com.benchmark.web.service.ResultService resultService,
                                        ExerciseRunner exerciseRunner,
                                        Config config,
                                        ExecutorService benchmarkExecutor) {
        return new QueueProcessor(sessionManager, resultService, exerciseRunner, config, benchmarkExecutor);
    }

    /**
     * BenchmarkService bean - facade for all benchmark operations.
     */
    @Bean
    public com.benchmark.web.service.BenchmarkService benchmarkService(
            SessionManager sessionManager,
            BenchmarkExecutor benchmarkExecutor,
            QueueProcessor queueProcessor,
            com.benchmark.web.service.ResultService resultService,
            ExerciseRunner exerciseRunner) {
        return new com.benchmark.web.service.BenchmarkService(
                sessionManager, benchmarkExecutor, queueProcessor, resultService, exerciseRunner);
    }

    /**
     * Thread factory that creates daemon threads (don't block JVM shutdown).
     */
    private static class DaemonThreadFactory implements ThreadFactory {
        @Override
        public Thread newThread(Runnable r) {
            Thread t = Executors.defaultThreadFactory().newThread(r);
            t.setDaemon(true);
            return t;
        }
    }
}

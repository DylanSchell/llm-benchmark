package io.schell.llm.benchmark.web.config;

import io.schell.llm.benchmark.BenchmarkRunner;
import io.schell.llm.benchmark.config.Config;
import io.schell.llm.benchmark.config.ConfigLoader;
import io.schell.llm.benchmark.docker.DockerClient;
import io.schell.llm.benchmark.exercise.ExerciseRunner;
import io.schell.llm.benchmark.web.service.BenchmarkExecutor;
import io.schell.llm.benchmark.web.service.QueueProcessor;
import io.schell.llm.benchmark.web.service.SessionManager;
import jakarta.annotation.PreDestroy;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;
import org.springframework.scheduling.annotation.EnableAsync;
import org.springframework.scheduling.concurrent.ThreadPoolTaskExecutor;
import org.springframework.web.servlet.config.annotation.ResourceHandlerRegistry;
import org.springframework.web.servlet.config.annotation.WebMvcConfigurer;

import java.util.concurrent.Executor;

import java.nio.file.Paths;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.ThreadFactory;
import java.util.concurrent.TimeUnit;

/**
 * Web configuration for static resources and executors.
 */
@Configuration
@EnableAsync
public class WebConfig implements WebMvcConfigurer {
    private static final Logger logger = LoggerFactory.getLogger(WebConfig.class);

    private ExecutorService benchmarkExecutorService;

    @Override
    public void addResourceHandlers(ResourceHandlerRegistry registry) {
        registry.addResourceHandler("/static/**")
                .addResourceLocations("classpath:/static/");
    }

    /**
     * Gracefully shut down executor services on application close.
     * Note: QueueProcessor has its own @PreDestroy that should be called first.
     */
    @PreDestroy
    public void shutdown() {
        logger.info("Shutting down web configuration beans...");
        
        // Shut down the benchmark executor service
        if (benchmarkExecutorService != null && !benchmarkExecutorService.isShutdown()) {
            logger.info("Shutting down benchmark executor service...");
            benchmarkExecutorService.shutdown();
            try {
                if (!benchmarkExecutorService.awaitTermination(10, TimeUnit.SECONDS)) {
                    logger.warn("Executor service did not terminate in time, forcing shutdown");
                    benchmarkExecutorService.shutdownNow();
                }
            } catch (InterruptedException e) {
                logger.warn("Interrupted while shutting down executor service");
                benchmarkExecutorService.shutdownNow();
                Thread.currentThread().interrupt();
            }
        }
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
    public ExecutorService benchmarkExecutorService() {
        this.benchmarkExecutorService = Executors.newCachedThreadPool(new DaemonThreadFactory());
        return this.benchmarkExecutorService;
    }

    /**
     * ResultPersister bean - depends on OutputConfig from Config.
     */
    @Bean
    public io.schell.llm.benchmark.persistence.ResultPersister resultPersister(Config config) {
        return new io.schell.llm.benchmark.persistence.ResultPersister(config.getOutput());
    }

    /**
     * ResultService bean - depends on Config and ResultPersister.
     */
    @Bean
    public io.schell.llm.benchmark.web.service.ResultService resultService(Config config) {
        return new io.schell.llm.benchmark.web.service.ResultService(config);
    }

    /**
     * SessionManager bean.
     */
    @Bean
    public SessionManager sessionManager(Config config) {
        return new SessionManager(config);
    }

    /**
     * BenchmarkExecutor bean - depends on BenchmarkRunner and DockerClient.
     */
    @Bean
    public BenchmarkExecutor benchmarkExecutor(BenchmarkRunner benchmarkRunner, DockerClient dockerClient, Config config) {
        return new BenchmarkExecutor(benchmarkRunner, dockerClient, config);
    }

    /**
     * QueueProcessor bean - depends on other services.
     */
    @Bean
    public QueueProcessor queueProcessor(SessionManager sessionManager, 
                                        io.schell.llm.benchmark.web.service.ResultService resultService,
                                        ExerciseRunner exerciseRunner,
                                        Config config,
                                        ExecutorService benchmarkExecutorService,
                                        BenchmarkExecutor benchmarkExecutor,
                                        io.schell.llm.benchmark.BenchmarkRunner benchmarkRunner) {
        return new QueueProcessor(sessionManager, resultService, exerciseRunner, config, benchmarkExecutorService, benchmarkExecutor, benchmarkRunner);
    }

    /**
     * BenchmarkService bean - facade for all benchmark operations.
     */
    @Bean
    public io.schell.llm.benchmark.web.service.BenchmarkService benchmarkService(
            SessionManager sessionManager,
            QueueProcessor queueProcessor,
            io.schell.llm.benchmark.web.service.ResultService resultService,
            ExerciseRunner exerciseRunner) {
        return new io.schell.llm.benchmark.web.service.BenchmarkService(
                sessionManager, queueProcessor, resultService, exerciseRunner);
    }

    /**
     * Spring TaskExecutor for @Async methods.
     */
    @Bean(name = "taskExecutor")
    public Executor taskExecutor() {
        ThreadPoolTaskExecutor executor = new ThreadPoolTaskExecutor();
        executor.setCorePoolSize(5);
        executor.setMaxPoolSize(10);
        executor.setQueueCapacity(25);
        executor.setThreadNamePrefix("benchmark-async-");
        executor.setThreadFactory(new DaemonThreadFactory());
        executor.setWaitForTasksToCompleteOnShutdown(true);
        executor.setAwaitTerminationSeconds(10);
        executor.initialize();
        return executor;
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

package com.benchmark.misc;

public class CostCalculator {
    /**
     * Calculates the cost of generating tokens based on electricity cost,
     * hardware power consumption, and inference speed.
     *
     * @param electricityCostPerKwh Cost of electricity per kilowatt-hour (e.g., 0.25 for €0.25/kWh)
     * @param tokensPerSecond       Inference speed in tokens per second (e.g., 20 t/s)
     * @param powerConsumptionWatts Hardware power consumption in watts (e.g., 150W)
     * @param numberOfTokens        Total number of tokens to generate (e.g., 1_000_000)
     * @return Total cost in the same currency as electricityCostPerKwh
     */
    public static double calculateTokenGenerationCost(
            double electricityCostPerKwh,
            double tokensPerSecond,
            double powerConsumptionWatts,
            long numberOfTokens
    ) {
        if (tokensPerSecond <= 0) {
            throw new IllegalArgumentException("Tokens per second must be greater than zero.");
        }
        if (powerConsumptionWatts < 0) {
            throw new IllegalArgumentException("Power consumption cannot be negative.");
        }
        if (numberOfTokens < 0) {
            throw new IllegalArgumentException("Number of tokens cannot be negative.");
        }

        // Time required in seconds
        double timeSeconds = (double) numberOfTokens / tokensPerSecond;

        // Convert time to hours
        double timeHours = timeSeconds / 3600.0;

        // Convert power to kilowatts
        double powerKilowatts = powerConsumptionWatts / 1000.0;

        // Energy consumed in kWh
        double energyKwh = powerKilowatts * timeHours;

        // Total cost
        return energyKwh * electricityCostPerKwh;
    }

    public static void main(String[] args) {
        // cost per 1M tokens for Qwen 3.5 122B-A3B on my mac studio
        double cost = CostCalculator.calculateTokenGenerationCost(0.25,34,150,1000_000);
        System.out.println("Cost per 1M tokens for Qwen 3.5 122B-A3B-Q8_0: "+cost);
        cost = CostCalculator.calculateTokenGenerationCost(0.25,68,150,1000_000);
        System.out.println("Cost per 1M tokens output for Qwen 3.5 35B-A3B-Q8_0: "+cost);
        cost = CostCalculator.calculateTokenGenerationCost(0.25,1800,150,1000_000);
        System.out.println("Cost per 1M tokens input for Qwen 3.5 35B-A3B-Q8_0: "+cost);
        // mac-mini 56W 44t/s on qwen 35b
        cost = CostCalculator.calculateTokenGenerationCost(0.25,778,83,1000_000);
        System.out.println("Cost per 1M tokens input for Qwen 3.6 35B on mac mini m4: "+cost);
        cost = CostCalculator.calculateTokenGenerationCost(0.25,45,56,1000_000);
        System.out.println("Cost per 1M  tokens output for Qwen 3.6 35B on mac mini m4: "+cost);
    }
}

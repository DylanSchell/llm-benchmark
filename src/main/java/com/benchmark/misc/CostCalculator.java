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

}

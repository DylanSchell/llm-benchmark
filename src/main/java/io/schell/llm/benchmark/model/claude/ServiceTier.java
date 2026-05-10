package io.schell.llm.benchmark.model.claude;

/**
 * Typed representation of the {@code service_tier} field.
 * The exact structure is not yet known; fields will be added as they are discovered.
 */
public class ServiceTier {
    // TODO: add concrete fields when the JSON structure is known
    private String serviceTier;

    public ServiceTier(String serviceTier) {
        this.serviceTier = serviceTier;
    }

    public String getServiceTier() {
        return serviceTier;
    }

    public void setServiceTier(String serviceTier) {
        this.serviceTier = serviceTier;
    }

    @Override
    public String toString() {
        return serviceTier;
    }
}

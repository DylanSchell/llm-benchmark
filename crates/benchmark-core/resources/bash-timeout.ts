import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";
import { isToolCallEventType } from "@mariozechner/pi-coding-agent";

interface BashTimeoutConfig {
    defaultTimeout: number; // 0 = infinite (no default), >0 = seconds
    maxTimeout: number;     // 0 = infinite (no cap), >0 = cap in seconds
}

export default function bashTimeout(pi: ExtensionAPI) {
    // Load persisted config
    let config = { defaultTimeout: 120, maxTimeout: 0 };

    // Tool call interceptor: inject default / cap max timeout
    pi.on("tool_call", (event) => {
        if (!isToolCallEventType("bash", event)) return;

        const input = event.input as { command: string; timeout?: number };

        let desired = input.timeout;

        // Apply default if timeout is undefined
        if (desired === undefined) {
            if (config.defaultTimeout > 0) {
                input.timeout = config.defaultTimeout;
                desired = config.defaultTimeout;
            }
        }

        // Apply max cap
        if (config.maxTimeout > 0 && desired !== undefined && desired > config.maxTimeout) {
            input.timeout = config.maxTimeout;
        }
    });

}
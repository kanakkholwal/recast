
export type User = {
    // --- Identification & Telemetry ---
    installId: string;     // Persistent UUID generated on first run
    sessionId: string;     // Ephemeral UUID regenerated each app launch
    deviceId: string;      // Hardware-linked device ID (if available via Tauri)
    
    // --- Account Info ---
    userId: string | null; // Database user ID (null for anonymous users)
    username: string | null;

    user_type: 'anonymous' | 'registered';
    user_plan: 'oss' | 'pro';
    
    // --- Context properties ---
    userAgent: string;
    appVersion: string;
    osPlatform: string;

    // --- App state ---
    // Whether the user has seen the "What's New" modal for the current version.
    seen_whats_new: string[];
};

export function createUserStore() {
    // Attempt to load an existing install ID to track returning anonymous users,
    // or generate a new persistent one.
    const storedInstallId = typeof localStorage !== 'undefined' ? localStorage.getItem('trace_install_id') : null;
    const installId = storedInstallId || crypto.randomUUID();
    
    if (typeof localStorage !== 'undefined' && !storedInstallId) {
        localStorage.setItem('trace_install_id', installId);
    }

    let user = $state<User>({
        installId,
        sessionId: crypto.randomUUID(),
        deviceId: '',
        userId: null,
        username: null,
        user_type: 'anonymous',
        user_plan: 'oss',
        userAgent: typeof navigator !== 'undefined' ? navigator.userAgent : 'unknown',
        appVersion: 'unknown',  // Hydrated later
        osPlatform: 'unknown',  // Hydrated later
        seen_whats_new: [],
    });

    return { 
        get user() { return user; },
        set user(v) { user = v; },
        
        /**
         * Helper to grab stripped-down, safe identification payload 
         * for analytics & telemetry services.
         */
        getTelemetryIdentity() {
            return {
                installId: user.installId,
                sessionId: user.sessionId,
                deviceId: user.deviceId,
                userId: user.userId,
                userType: user.user_type,
                userPlan: user.user_plan,
                appVersion: user.appVersion,
                osPlatform: user.osPlatform
            };
        }
    };
}
/**
 * Theme Initialization (prevent flash)
 * P0 SECURITY FIX: Moved from inline script to external file for strict CSP
 * Must load in <head> before page renders
 */
(function() {
    const theme = localStorage.getItem('theme') || 'dark';
    if (theme === 'light') {
        document.documentElement.classList.remove('dark');
    } else if (theme === 'system') {
        if (!window.matchMedia('(prefers-color-scheme: dark)').matches) {
            document.documentElement.classList.remove('dark');
        }
    }
    // Default: dark theme (already set via class="dark")
})();

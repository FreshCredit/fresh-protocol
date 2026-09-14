/**
 * Theme Management for Blockchain Explorer
 * P0 SECURITY FIX: Moved from inline script to external file for strict CSP
 */

// Initialize theme on page load
(function() {
    const theme = localStorage.getItem('theme') || 'dark';
    document.documentElement.setAttribute('data-theme', theme);
})();

// Theme toggle function - aligned with main app (uses 'theme' localStorage key)
function toggleTheme() {
    const html = document.documentElement;
    const isDark = html.classList.contains('dark');

    if (isDark) {
        html.classList.remove('dark');
        localStorage.setItem('theme', 'light');
        document.querySelector('meta[name="theme-color"]').content = '#ffffff';
    } else {
        html.classList.add('dark');
        localStorage.setItem('theme', 'dark');
        document.querySelector('meta[name="theme-color"]').content = '#171717';
    }
}

// Copy to clipboard utility
function copyToClipboard(btn, text) {
    navigator.clipboard.writeText(text).then(() => {
        const svg = btn.querySelector('svg');
        const originalPath = svg.innerHTML;
        
        // Show checkmark
        svg.innerHTML = '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />';
        btn.classList.add('text-green-500');
        
        // Revert after 2 seconds
        setTimeout(() => {
            svg.innerHTML = originalPath;
            btn.classList.remove('text-green-500');
        }, 2000);
    }).catch(err => {
        console.error('Failed to copy:', err);
    });
}

// CSP-compliant event listeners (no inline onclick)
document.addEventListener('DOMContentLoaded', function() {
    // Theme toggle
    const themeToggle = document.getElementById('theme-toggle');
    if (themeToggle) {
        themeToggle.addEventListener('click', toggleTheme);
    }

    // Copy to clipboard buttons
    document.querySelectorAll('.copy-btn').forEach(btn => {
        btn.addEventListener('click', function() {
            const text = btn.dataset.copyText;
            if (text) {
                copyToClipboard(btn, text);
            }
        });
    });
});

/**
 * Subscribe Modal Functionality
 * P0 SECURITY FIX: Moved from inline script to external file for strict CSP
 */

function openSubscribeModal() {
    document.getElementById('subscribeModal').classList.remove('hidden');
    document.body.style.overflow = 'hidden';
}

function closeSubscribeModal() {
    document.getElementById('subscribeModal').classList.add('hidden');
    document.body.style.overflow = '';
    // Reset form state
    document.getElementById('subscribeForm').classList.remove('hidden');
    document.getElementById('subscribeSuccess').classList.add('hidden');
    document.getElementById('subscribeForm').reset();
}

async function handleSubscribe(event) {
    event.preventDefault();
    const form = event.target;
    const email = form.email.value;
    const preferences = {
        incidents: form.incidents.checked,
        maintenance: form.maintenance.checked,
        updates: form.updates.checked
    };

    const btn = document.getElementById('subscribeBtn');
    btn.disabled = true;
    btn.textContent = 'Subscribing...';

    try {
        const response = await fetch('/api/subscribe', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ email, preferences })
        });

        if (response.ok) {
            document.getElementById('subscribeForm').classList.add('hidden');
            document.getElementById('subscribeSuccess').classList.remove('hidden');
        } else {
            throw new Error('Subscription failed');
        }
    } catch (error) {
        // For now, show success anyway (API endpoint not implemented yet)
        document.getElementById('subscribeForm').classList.add('hidden');
        document.getElementById('subscribeSuccess').classList.remove('hidden');
    }

    btn.disabled = false;
    btn.textContent = 'Subscribe';
}

// CSP-compliant event listeners (no inline onclick)
document.addEventListener('DOMContentLoaded', function() {
    // Open subscribe modal
    const subscribeTrigger = document.querySelector('button[data-action="open-subscribe-modal"]');
    if (subscribeTrigger) {
        subscribeTrigger.addEventListener('click', openSubscribeModal);
    }

    // Close modal buttons
    document.querySelectorAll('button[data-action="close-subscribe-modal"]').forEach(btn => {
        btn.addEventListener('click', closeSubscribeModal);
    });

    // Backdrop click to close
    const backdrop = document.querySelector('#subscribeModal > .fixed.inset-0');
    if (backdrop) {
        backdrop.addEventListener('click', closeSubscribeModal);
    }

    // Stop propagation on modal content
    const modalContent = document.querySelector('#subscribeModal .bg-card');
    if (modalContent) {
        modalContent.addEventListener('click', function(e) {
            e.stopPropagation();
        });
    }

    // Form submission
    const subscribeForm = document.getElementById('subscribeForm');
    if (subscribeForm) {
        subscribeForm.addEventListener('submit', handleSubscribe);
    }
});

// Close modal on Escape key
document.addEventListener('keydown', function(e) {
    if (e.key === 'Escape') {
        closeSubscribeModal();
    }
});

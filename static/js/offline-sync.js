/**
 * Offline sync module.
 *
 * Handles automatic detection of network status and syncing
 * offline study progress when back online.
 *
 * Also handles background session refresh to keep a fresh
 * offline session always ready.
 */

'use strict';

const OfflineSync = (function() {
  let syncInProgress = false;
  let refreshInProgress = false;
  let notificationElement = null;
  let stabilityTimer = null;
  let syncPromptModal = null;

  // Session is considered stale after 4 hours
  const SESSION_STALE_HOURS = 4;
  // Default session duration in minutes
  const DEFAULT_SESSION_MINUTES = 30;
  // Wait for connection to be stable before prompting (configurable for tests)
  let STABILITY_DELAY_MS = 5000;

  /**
   * Check if we're online.
   * Allows test API to override isOnline() for E2E tests.
   * @returns {boolean}
   */
  function isOnline() {
    if (window.OfflineSyncTestAPI && window.OfflineSyncTestAPI._testOnlineState !== null) {
      return window.OfflineSyncTestAPI._testOnlineState;
    }
    return navigator.onLine;
  }

  /**
   * Check if session is stale (older than SESSION_STALE_HOURS).
   * @param {Object} session - Session object with created_at timestamp
   * @returns {boolean}
   */
  function isSessionStale(session) {
    if (!session || !session.created_at) {
      return true;
    }
    var createdAt = new Date(session.created_at);
    var now = new Date();
    var hoursDiff = (now - createdAt) / (1000 * 60 * 60);
    return hoursDiff > SESSION_STALE_HOURS;
  }

  /**
   * Refresh the offline study session in the background.
   * Downloads a fresh session from the server and saves to IndexedDB.
   * @returns {Promise<{success: boolean, error?: string}>}
   */
  async function refreshSession() {
    if (refreshInProgress) {
      return { success: false, error: 'Refresh already in progress' };
    }

    if (!isOnline()) {
      return { success: false, error: 'Offline' };
    }

    refreshInProgress = true;

    try {
      // Download new session from server
      var response = await fetch('/api/study/download-session', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json'
        },
        body: JSON.stringify({
          duration_minutes: DEFAULT_SESSION_MINUTES,
          filter_mode: 'all'
        }),
        credentials: 'same-origin'
      });

      if (response.status === 403) {
        // Offline mode not enabled - this is expected, silently skip
        refreshInProgress = false;
        return { success: false, error: 'Offline mode not enabled' };
      }

      if (!response.ok) {
        var errorData = await response.json().catch(function() {
          return { error: 'Unknown error' };
        });
        refreshInProgress = false;
        return { success: false, error: errorData.error || 'Server error' };
      }

      var session = await response.json();

      // Save to IndexedDB
      if (window.OfflineStorage && typeof window.OfflineStorage.saveSession === 'function') {
        await window.OfflineStorage.saveSession(session);
        console.log('[OfflineSync] Session refreshed:', session.cards ? session.cards.length : 0, 'cards');
      }

      refreshInProgress = false;
      return { success: true };

    } catch (error) {
      refreshInProgress = false;
      console.warn('[OfflineSync] Session refresh failed:', error.message);
      return { success: false, error: error.message || 'Network error' };
    }
  }

  /**
   * Check if we need a fresh session and refresh if needed.
   * Called automatically on page load when online.
   */
  async function checkAndRefreshSession() {
    // Don't refresh if offline
    if (!isOnline()) {
      return;
    }

    // Check if OfflineStorage is available
    if (!window.OfflineStorage || typeof window.OfflineStorage.getSession !== 'function') {
      console.log('[OfflineSync] OfflineStorage not available, skipping refresh check');
      return;
    }

    try {
      var session = await window.OfflineStorage.getSession();

      // Refresh if no session or session is stale
      if (!session || isSessionStale(session)) {
        console.log('[OfflineSync] Session', session ? 'stale' : 'missing', '- refreshing in background...');
        // Don't await - let it run in background
        refreshSession();
      } else {
        console.log('[OfflineSync] Session is fresh, no refresh needed');
      }
    } catch (e) {
      console.warn('[OfflineSync] Could not check session:', e);
    }
  }

  /**
   * Check if there are pending reviews to sync.
   * @returns {Promise<boolean>}
   */
  async function hasPendingSync() {
    try {
      const count = await window.OfflineStorage.getPendingCount();
      return count > 0;
    } catch (e) {
      return false;
    }
  }

  /**
   * Parse card ID from error string.
   * Error format: "Card 42: error message"
   * @param {string} error - Error string from server
   * @returns {number|null} Card ID or null if couldn't parse
   */
  function parseCardIdFromError(error) {
    var match = error.match(/^Card\s+(\d+):/);
    if (match) {
      return parseInt(match[1], 10);
    }
    return null;
  }

  /**
   * Sync offline progress to the server.
   * @returns {Promise<{success: boolean, synced_count: number, total_count: number, errors: string[]}>}
   */
  async function syncProgress() {
    if (syncInProgress) {
      return { success: false, synced_count: 0, total_count: 0, errors: ['Sync already in progress'] };
    }

    syncInProgress = true;

    try {
      // Prepare sync payload
      const payload = await window.OfflineStorage.prepareSyncPayload();
      const totalCount = payload.reviews.length;

      if (totalCount === 0) {
        syncInProgress = false;
        return { success: true, synced_count: 0, total_count: 0, errors: [] };
      }

      // Send to server
      const response = await fetch('/api/study/sync-offline', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json'
        },
        body: JSON.stringify(payload),
        credentials: 'same-origin'
      });

      if (!response.ok) {
        const errorData = await response.json().catch(function() {
          return { error: 'Unknown error' };
        });
        syncInProgress = false;
        return {
          success: false,
          synced_count: 0,
          total_count: totalCount,
          errors: [errorData.error || 'Server error']
        };
      }

      const result = await response.json();
      const errors = result.errors || [];

      // Parse failed card IDs from errors
      var failedCardIds = [];
      errors.forEach(function(err) {
        var cardId = parseCardIdFromError(err);
        if (cardId !== null) {
          failedCardIds.push(cardId);
        }
      });

      // Clear only successfully synced responses
      // If all synced (no errors), clear everything
      // If some failed, keep only the failed ones for retry
      if (result.synced_count > 0 || errors.length === 0) {
        await window.OfflineStorage.clearSyncedResponses(failedCardIds);
      }

      syncInProgress = false;
      return {
        success: errors.length === 0,
        synced_count: result.synced_count,
        skipped_count: result.skipped_count || 0,
        skipped_cards: result.skipped_cards || [],
        total_count: totalCount,
        errors: errors
      };

    } catch (error) {
      syncInProgress = false;
      return {
        success: false,
        synced_count: 0,
        total_count: 0,
        errors: [error.message || 'Network error']
      };
    }
  }

  /**
   * Show sync modal (loading state).
   * @param {number} pendingCount - Number of reviews pending sync
   */
  function showSyncModal(pendingCount) {
    // Remove existing notification
    if (notificationElement && notificationElement.parentNode) {
      notificationElement.parentNode.removeChild(notificationElement);
    }

    notificationElement = document.createElement('div');
    notificationElement.id = 'offline-sync-notification';
    notificationElement.setAttribute('role', 'alert');
    notificationElement.setAttribute('aria-live', 'polite');

    notificationElement.className = [
      'fixed', 'bottom-4', 'left-4', 'right-4',
      'sm:left-auto', 'sm:right-4', 'sm:w-80',
      'bg-white', 'dark:bg-gray-800',
      'border', 'border-gray-200', 'dark:border-gray-700',
      'px-4', 'py-3', 'rounded-lg', 'shadow-lg',
      'z-50'
    ].join(' ');

    notificationElement.innerHTML = `
      <div class="flex items-center gap-3">
        <svg class="w-5 h-5 text-indigo-600 dark:text-indigo-400 animate-spin" fill="none" viewBox="0 0 24 24">
          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
          <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
        </svg>
        <div>
          <p class="font-medium text-gray-900 dark:text-white">Syncing offline progress...</p>
          <p class="text-sm text-gray-500 dark:text-gray-400">${pendingCount} review${pendingCount === 1 ? '' : 's'}</p>
        </div>
      </div>
    `;

    document.body.appendChild(notificationElement);
  }

  /**
   * Show sync result and auto-dismiss.
   * @param {Object} result - Sync result
   */
  function showSyncResult(result) {
    if (!notificationElement) return;

    // Full success - all synced (with possible skipped due to conflicts)
    if (result.success && result.synced_count > 0) {
      var skippedInfo = result.skipped_count > 0
        ? `<p class="text-sm text-gray-500 dark:text-gray-400">${result.skipped_count} skipped (already reviewed online)</p>`
        : '';
      notificationElement.innerHTML = `
        <div class="flex items-center gap-3">
          <svg class="w-5 h-5 text-green-600 dark:text-green-400" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" d="M9 12.75L11.25 15 15 9.75M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          <div>
            <p class="font-medium text-gray-900 dark:text-white">
              Synced ${result.synced_count} review${result.synced_count === 1 ? '' : 's'}
            </p>
            ${skippedInfo}
          </div>
        </div>
      `;
      setTimeout(hideNotification, result.skipped_count > 0 ? 3000 : 2000);
    } else if (result.synced_count === 0 && result.total_count === 0) {
      // Nothing to sync, just hide
      hideNotification();
    } else if (result.synced_count === 0 && result.skipped_count > 0 && result.errors.length === 0) {
      // All reviews were skipped due to conflicts (already reviewed online)
      notificationElement.innerHTML = `
        <div class="flex items-center gap-3">
          <svg class="w-5 h-5 text-blue-600 dark:text-blue-400" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" d="M11.25 11.25l.041-.02a.75.75 0 011.063.852l-.708 2.836a.75.75 0 001.063.853l.041-.021M21 12a9 9 0 11-18 0 9 9 0 0118 0zm-9-3.75h.008v.008H12V8.25z" />
          </svg>
          <div>
            <p class="font-medium text-gray-900 dark:text-white">
              All ${result.skipped_count} reviews skipped
            </p>
            <p class="text-sm text-gray-500 dark:text-gray-400">
              Cards were already reviewed online
            </p>
          </div>
        </div>
      `;
      // Clear the skipped reviews from storage since they don't need retry
      window.OfflineStorage.clearAll();
      setTimeout(hideNotification, 3000);
    } else if (result.synced_count > 0 && (result.errors.length > 0 || result.skipped_count > 0)) {
      // Partial success - some synced, some failed or skipped
      var failedCount = result.errors.length;
      var statusParts = [];
      if (result.skipped_count > 0) {
        statusParts.push(`${result.skipped_count} skipped (reviewed online)`);
      }
      if (failedCount > 0) {
        statusParts.push(`${failedCount} failed - will retry`);
      }
      notificationElement.innerHTML = `
        <div class="flex items-center gap-3">
          <svg class="w-5 h-5 text-yellow-600 dark:text-yellow-400" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z" />
          </svg>
          <div>
            <p class="font-medium text-gray-900 dark:text-white">
              Synced ${result.synced_count} of ${result.total_count} reviews
            </p>
            <p class="text-sm text-gray-500 dark:text-gray-400">
              ${statusParts.join(', ')}
            </p>
          </div>
        </div>
      `;
      setTimeout(hideNotification, 4000);
    } else {
      // Complete failure - nothing synced
      notificationElement.innerHTML = `
        <div class="flex items-center gap-3">
          <svg class="w-5 h-5 text-red-600 dark:text-red-400" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m9-.75a9 9 0 11-18 0 9 9 0 0118 0zm-9 3.75h.008v.008H12v-.008z" />
          </svg>
          <div>
            <p class="font-medium text-gray-900 dark:text-white">Sync failed</p>
            <p class="text-sm text-gray-500 dark:text-gray-400">
              ${result.errors.length > 0 ? result.errors[0] : 'Please try again later'}
            </p>
          </div>
        </div>
      `;
      setTimeout(hideNotification, 4000);
    }
  }

  /**
   * Perform auto-sync with modal.
   * @param {number} pendingCount - Number of reviews pending sync
   */
  async function performAutoSync(pendingCount) {
    showSyncModal(pendingCount);
    var result = await syncProgress();
    showSyncResult(result);
  }

  /**
   * Hide the sync notification.
   */
  function hideNotification() {
    if (notificationElement && notificationElement.parentNode) {
      notificationElement.style.transition = 'opacity 0.3s ease-out';
      notificationElement.style.opacity = '0';
      setTimeout(function() {
        if (notificationElement && notificationElement.parentNode) {
          notificationElement.parentNode.removeChild(notificationElement);
        }
        notificationElement = null;
      }, 300);
    }
  }

  /**
   * Check for pending sync (legacy function, kept for compatibility).
   * Now shows prompt instead of auto-syncing.
   */
  async function checkAndNotify() {
    // Don't sync if offline
    if (!isOnline()) {
      return;
    }

    // Check for pending reviews
    var pending = await hasPendingSync();
    if (!pending) {
      return;
    }

    // Get count and show prompt (instead of auto-sync)
    var count = await window.OfflineStorage.getPendingCount();
    if (count > 0) {
      showSyncPromptModal(count);
    }
  }

  /**
   * Create and show the sync prompt modal.
   * @param {number} pendingCount - Number of reviews pending sync
   */
  function showSyncPromptModal(pendingCount) {
    // Don't show if already showing
    if (syncPromptModal && document.body.contains(syncPromptModal)) {
      // Update count if modal already exists
      var countEl = syncPromptModal.querySelector('#sync-prompt-count');
      if (countEl) countEl.textContent = pendingCount;
      return;
    }

    // Create modal
    syncPromptModal = document.createElement('div');
    syncPromptModal.id = 'sync-prompt-modal';
    syncPromptModal.className = 'fixed inset-0 bg-black/50 flex items-center justify-center z-50';
    syncPromptModal.setAttribute('role', 'dialog');
    syncPromptModal.setAttribute('aria-modal', 'true');
    syncPromptModal.setAttribute('aria-labelledby', 'sync-prompt-title');

    syncPromptModal.innerHTML = `
      <div class="bg-white dark:bg-gray-800 rounded-lg p-6 max-w-sm mx-4 shadow-xl">
        <h3 id="sync-prompt-title" class="text-lg font-semibold text-gray-900 dark:text-white mb-2">Back Online</h3>
        <p class="text-gray-600 dark:text-gray-300 mb-4">
          You have <span id="sync-prompt-count" class="font-semibold text-indigo-600 dark:text-indigo-400">${pendingCount}</span> pending review${pendingCount === 1 ? '' : 's'}. Sync now?
        </p>
        <div class="flex gap-3">
          <button id="sync-now-btn" class="flex-1 bg-indigo-600 hover:bg-indigo-700 text-white py-2 px-4 rounded-lg font-medium transition-colors">
            Sync Now
          </button>
          <button id="stay-offline-btn" class="flex-1 bg-gray-200 hover:bg-gray-300 dark:bg-gray-700 dark:hover:bg-gray-600 text-gray-800 dark:text-gray-200 py-2 px-4 rounded-lg font-medium transition-colors">
            Stay Offline
          </button>
        </div>
      </div>
    `;

    document.body.appendChild(syncPromptModal);

    // Bind button handlers
    syncPromptModal.querySelector('#sync-now-btn').addEventListener('click', async function() {
      hideSyncPromptModal();
      await performAutoSync(pendingCount);
      // Refresh session after successful sync
      checkAndRefreshSession();
    });

    syncPromptModal.querySelector('#stay-offline-btn').addEventListener('click', function() {
      hideSyncPromptModal();
      console.log('[OfflineSync] User chose to stay offline');
    });

    // Close on backdrop click
    syncPromptModal.addEventListener('click', function(e) {
      if (e.target === syncPromptModal) {
        hideSyncPromptModal();
      }
    });

    // Close on Escape key
    function handleEscape(e) {
      if (e.key === 'Escape') {
        hideSyncPromptModal();
        document.removeEventListener('keydown', handleEscape);
      }
    }
    document.addEventListener('keydown', handleEscape);
  }

  /**
   * Hide the sync prompt modal.
   */
  function hideSyncPromptModal() {
    if (syncPromptModal && syncPromptModal.parentNode) {
      syncPromptModal.parentNode.removeChild(syncPromptModal);
    }
    syncPromptModal = null;
  }

  /**
   * Check if sync prompt modal is visible.
   * @returns {boolean}
   */
  function isSyncPromptVisible() {
    return syncPromptModal !== null && document.body.contains(syncPromptModal);
  }

  /**
   * Handle coming back online with stability check.
   * Waits for connection to be stable before prompting.
   */
  function handleOnline() {
    // Check for test API override of stability delay
    var delay = (window.OfflineSyncTestAPI && window.OfflineSyncTestAPI._stabilityDelayMs !== undefined)
      ? window.OfflineSyncTestAPI._stabilityDelayMs
      : STABILITY_DELAY_MS;

    console.log('[OfflineSync] Online event - starting stability timer (' + delay + 'ms)');

    // Cancel any existing timer
    if (stabilityTimer) {
      clearTimeout(stabilityTimer);
    }

    // Update test API state
    if (window.OfflineSyncTestAPI) {
      window.OfflineSyncTestAPI._isTimerActive = true;
    }

    stabilityTimer = setTimeout(async function() {
      stabilityTimer = null;
      // Update test API state
      if (window.OfflineSyncTestAPI) {
        window.OfflineSyncTestAPI._isTimerActive = false;
      }

      console.log('[OfflineSync] Connection stable, checking pending...');

      var count = await window.OfflineStorage.getPendingCount();
      if (count > 0) {
        showSyncPromptModal(count);
      } else {
        // No pending reviews, silently refresh session if stale
        checkAndRefreshSession();
      }
    }, delay);
  }

  /**
   * Handle going offline - cancel stability timer.
   */
  function handleOffline() {
    console.log('[OfflineSync] Offline event - canceling stability timer');
    if (stabilityTimer) {
      clearTimeout(stabilityTimer);
      stabilityTimer = null;
    }
    // Update test API state
    if (window.OfflineSyncTestAPI) {
      window.OfflineSyncTestAPI._isTimerActive = false;
    }
    // Also hide sync prompt if showing
    hideSyncPromptModal();
  }

  /**
   * Initialize the sync module.
   * Sets up event listeners for online/offline events.
   */
  function init() {
    // Listen for online event - use stability timer
    window.addEventListener('online', handleOnline);

    // Listen for offline event - cancel timer
    window.addEventListener('offline', handleOffline);

    // Check on page load (in case we're already online with pending sync)
    if (document.readyState === 'loading') {
      document.addEventListener('DOMContentLoaded', function() {
        if (isOnline()) {
          // Use stability timer on page load too
          handleOnline();
        }
      });
    } else {
      if (isOnline()) {
        // Use stability timer on page load too
        handleOnline();
      }
    }

    console.log('[OfflineSync] Initialized');
  }

  // Public API
  return {
    init: init,
    hasPendingSync: hasPendingSync,
    syncProgress: syncProgress,
    checkAndNotify: checkAndNotify,
    refreshSession: refreshSession,
    checkAndRefreshSession: checkAndRefreshSession,
    isSessionStale: isSessionStale,
    showSyncPromptModal: showSyncPromptModal,
    hideSyncPromptModal: hideSyncPromptModal,
    isSyncPromptVisible: isSyncPromptVisible
  };
})();

// Initialize on load
OfflineSync.init();

// Export for use in other modules
window.OfflineSync = OfflineSync;

/**
 * Test API for offline sync module.
 * Allows programmatic control of network state simulation for E2E tests.
 */
window.OfflineSyncTestAPI = {
  /**
   * Simulate coming online (triggers stability timer → prompt).
   * Also sets test-mode online state so isOnline() returns true.
   */
  simulateOnline: function() {
    window.OfflineSyncTestAPI._testOnlineState = true;
    window.dispatchEvent(new Event('online'));
  },

  /**
   * Simulate going offline (cancels stability timer).
   * Also sets test-mode online state so isOnline() returns false.
   */
  simulateOffline: function() {
    window.OfflineSyncTestAPI._testOnlineState = false;
    window.dispatchEvent(new Event('offline'));
  },

  /**
   * Set stability delay for faster tests.
   * @param {number} ms - Delay in milliseconds
   */
  setStabilityDelay: function(ms) {
    // Access the module's internal variable via closure workaround
    // We need to re-initialize with new delay
    window.OfflineSyncTestAPI._stabilityDelayMs = ms;
  },

  /**
   * Check if sync prompt modal is visible.
   * @returns {boolean}
   */
  isSyncPromptVisible: function() {
    return window.OfflineSync.isSyncPromptVisible();
  },

  /**
   * Get pending count shown in modal.
   * @returns {number}
   */
  getSyncPromptCount: function() {
    var el = document.getElementById('sync-prompt-count');
    return el ? parseInt(el.textContent, 10) : 0;
  },

  /**
   * Click sync now button.
   */
  clickSyncNow: function() {
    var btn = document.getElementById('sync-now-btn');
    if (btn) btn.click();
  },

  /**
   * Click stay offline button.
   */
  clickStayOffline: function() {
    var btn = document.getElementById('stay-offline-btn');
    if (btn) btn.click();
  },

  /**
   * Force show sync prompt (bypass stability timer).
   * @returns {Promise<void>}
   */
  forceShowSyncPrompt: async function() {
    var count = await window.OfflineStorage.getPendingCount();
    window.OfflineSync.showSyncPromptModal(count > 0 ? count : 1);
  },

  /**
   * Check if stability timer is currently active.
   * @returns {boolean}
   */
  isStabilityTimerActive: function() {
    // Check by trying to read internal state
    // Since we can't directly access stabilityTimer, we use a workaround
    return window.OfflineSyncTestAPI._isTimerActive || false;
  },

  /**
   * Reset test API state.
   * Call between tests to ensure clean state.
   */
  reset: function() {
    window.OfflineSyncTestAPI._testOnlineState = null;
    window.OfflineSyncTestAPI._isTimerActive = false;
    window.OfflineSyncTestAPI._stabilityDelayMs = 5000;
  },

  // Internal tracking for test API
  _stabilityDelayMs: 5000,
  _isTimerActive: false,
  _testOnlineState: null  // null = use real navigator.onLine, true/false = override
};

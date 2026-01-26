/**
 * Backend reachability check module.
 *
 * Determines if the backend server is reachable, independent of general
 * internet connectivity. This handles the case where a user is offline
 * from the internet but has a local server running.
 *
 * Strategy:
 * - If navigator.onLine is true → we're online, no ping needed
 * - If navigator.onLine is false → ping backend to verify reachability
 *
 * Works in both main thread and service worker contexts.
 */

'use strict';

var BackendPing = (function() {
  var HEALTH_ENDPOINT = '/api/health';
  var PING_TIMEOUT_MS = 3000; // 3 second timeout

  // Cache the last known state to avoid excessive pings
  var lastPingResult = null;
  var lastPingTime = 0;
  var CACHE_DURATION_MS = 5000; // Cache result for 5 seconds

  /**
   * Check if the backend server is reachable.
   * Uses navigator.onLine as fast first check, then pings backend if needed.
   *
   * @param {Object} options
   * @param {boolean} options.useCache - Whether to use cached result (default: true)
   * @param {number} options.timeout - Timeout in ms (default: PING_TIMEOUT_MS)
   * @param {boolean} options.forcePing - Force ping even if navigator.onLine is true (default: false)
   * @returns {Promise<boolean>} - true if backend is reachable
   */
  function isBackendReachable(options) {
    options = options || {};
    var useCache = options.useCache !== false;
    var timeout = options.timeout || PING_TIMEOUT_MS;
    var forcePing = options.forcePing === true;

    // Fast path: if browser says online and we're not forcing a ping, trust it
    if (navigator.onLine && !forcePing) {
      return Promise.resolve(true);
    }

    // Check cache first (only when navigator.onLine is false)
    if (useCache && lastPingResult !== null) {
      var elapsed = Date.now() - lastPingTime;
      if (elapsed < CACHE_DURATION_MS) {
        return Promise.resolve(lastPingResult);
      }
    }

    // navigator.onLine is false (or forcePing) - ping backend to verify
    return pingBackend(timeout);
  }

  /**
   * Actually ping the backend health endpoint.
   * @param {number} timeout - Timeout in milliseconds
   * @returns {Promise<boolean>}
   */
  function pingBackend(timeout) {
    return new Promise(function(resolve) {
      var controller = new AbortController();
      var timeoutId = setTimeout(function() {
        controller.abort();
      }, timeout);

      fetch(HEALTH_ENDPOINT, {
        method: 'HEAD',
        signal: controller.signal,
        cache: 'no-store',
        credentials: 'omit' // No cookies needed for health check
      }).then(function(response) {
        clearTimeout(timeoutId);
        var reachable = response.ok;
        lastPingResult = reachable;
        lastPingTime = Date.now();
        resolve(reachable);
      }).catch(function() {
        clearTimeout(timeoutId);
        // Network error, timeout, or abort - backend not reachable
        lastPingResult = false;
        lastPingTime = Date.now();
        resolve(false);
      });
    });
  }

  /**
   * Invalidate the cached ping result.
   * Call this when network state changes.
   */
  function invalidateCache() {
    lastPingResult = null;
    lastPingTime = 0;
  }

  /**
   * Synchronous check using cached state only.
   * Returns cached result or navigator.onLine as fallback.
   *
   * Important: This function ONLY uses cached state. Call isBackendReachable()
   * proactively (on SW activate, on online events) to populate the cache.
   *
   * Cache TTL is extended for sync checks since we can't block on network.
   *
   * @returns {boolean}
   */
  function isReachableSync() {
    // Fast path: navigator.onLine is true → we're online
    if (navigator.onLine) {
      return true;
    }

    // navigator.onLine is false - check if we have cached proof the backend is reachable
    // Use extended TTL (30s) for sync checks to reduce false negatives
    var SYNC_CACHE_DURATION_MS = 30000;

    if (lastPingResult !== null) {
      var elapsed = Date.now() - lastPingTime;
      if (elapsed < SYNC_CACHE_DURATION_MS) {
        return lastPingResult;
      }
    }

    // No cached result or stale cache
    // Default to navigator.onLine (false) - we can't prove backend is reachable
    // The cache should be populated by:
    // 1. SW activate event calling isBackendReachable()
    // 2. Online events calling isBackendReachable()
    // 3. Normal async isOnline() calls from offline-sync.js
    return false;
  }

  return {
    isBackendReachable: isBackendReachable,
    isReachableSync: isReachableSync,
    invalidateCache: invalidateCache,
    // Expose constants for testing
    PING_TIMEOUT_MS: PING_TIMEOUT_MS,
    CACHE_DURATION_MS: CACHE_DURATION_MS
  };
})();

// Export for use in other modules (works in both window and service worker)
if (typeof window !== 'undefined') {
  window.BackendPing = BackendPing;
}
if (typeof self !== 'undefined') {
  self.BackendPing = BackendPing;
}

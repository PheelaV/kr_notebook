/**
 * Service Worker Registration Script
 *
 * Handles:
 * - Service worker registration on page load
 * - Update notifications with unobtrusive toast
 * - Public API for manual operations
 */

(function() {
  'use strict';

  // Only proceed if service workers are supported
  if (!('serviceWorker' in navigator)) {
    console.log('[SW Register] Service workers not supported');
    return;
  }

  var updateNotification = null;
  var registration = null;
  var precacheTriggered = false;

  /**
   * Show update notification toast
   */
  function showUpdateNotification() {
    // Remove existing notification if present
    if (updateNotification && updateNotification.parentNode) {
      updateNotification.parentNode.removeChild(updateNotification);
    }

    updateNotification = document.createElement('div');
    updateNotification.id = 'sw-update-notification';
    updateNotification.setAttribute('role', 'alert');
    updateNotification.setAttribute('aria-live', 'polite');

    // Tailwind classes for styling
    updateNotification.className = [
      'fixed', 'bottom-4', 'left-4', 'right-4',
      'sm:left-auto', 'sm:right-4', 'sm:w-80',
      'bg-indigo-600', 'text-white',
      'px-4', 'py-3', 'rounded-lg', 'shadow-lg',
      'z-50', 'flex', 'items-center', 'justify-between',
      'animate-fade-in'
    ].join(' ');

    updateNotification.innerHTML = [
      '<span class="text-sm">App update available</span>',
      '<div class="flex items-center gap-2 ml-4">',
      '  <button id="sw-update-refresh" class="px-3 py-1 bg-white text-indigo-600 rounded text-sm font-medium hover:bg-gray-100 transition-colors">',
      '    Refresh',
      '  </button>',
      '  <button id="sw-update-dismiss" class="p-1 text-indigo-200 hover:text-white transition-colors" aria-label="Dismiss">',
      '    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">',
      '      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>',
      '    </svg>',
      '  </button>',
      '</div>'
    ].join('');

    document.body.appendChild(updateNotification);

    // Add event listeners
    var refreshBtn = document.getElementById('sw-update-refresh');
    var dismissBtn = document.getElementById('sw-update-dismiss');

    if (refreshBtn) {
      refreshBtn.addEventListener('click', function() {
        // Tell waiting service worker to take over
        if (registration && registration.waiting) {
          registration.waiting.postMessage({ type: 'SKIP_WAITING' });
        }
        window.location.reload();
      });
    }

    if (dismissBtn) {
      dismissBtn.addEventListener('click', function() {
        hideUpdateNotification();
      });
    }

    // Auto-dismiss after 15 seconds
    setTimeout(function() {
      hideUpdateNotification();
    }, 15000);
  }

  /**
   * Hide update notification with fade out
   */
  function hideUpdateNotification() {
    if (updateNotification && updateNotification.parentNode) {
      updateNotification.style.transition = 'opacity 0.3s ease-out';
      updateNotification.style.opacity = '0';
      setTimeout(function() {
        if (updateNotification && updateNotification.parentNode) {
          updateNotification.parentNode.removeChild(updateNotification);
        }
        updateNotification = null;
      }, 300);
    }
  }

  /**
   * Track installing worker and show notification when ready
   */
  function trackInstalling(worker) {
    worker.addEventListener('statechange', function() {
      if (worker.state === 'installed' && navigator.serviceWorker.controller) {
        // New service worker installed and there's an existing controller
        // This means an update is available
        showUpdateNotification();
      }
    });
  }

  /**
   * Trigger precaching of reference pages
   */
  function triggerPrecache() {
    if (navigator.serviceWorker.controller) {
      console.log('[SW Register] Triggering precache of reference pages');
      navigator.serviceWorker.controller.postMessage({ type: 'PRECACHE_PAGES' });
    }
  }

  /**
   * Register the service worker
   */
  function registerServiceWorker() {
    navigator.serviceWorker.register('/sw.js', {
      scope: '/'
    }).then(function(reg) {
      registration = reg;
      console.log('[SW Register] Registered with scope:', reg.scope);

      // Check if there's an update waiting
      if (reg.waiting) {
        showUpdateNotification();
        return;
      }

      // Check if there's an update installing
      if (reg.installing) {
        trackInstalling(reg.installing);
        return;
      }

      // Listen for future updates
      reg.addEventListener('updatefound', function() {
        if (reg.installing) {
          trackInstalling(reg.installing);
        }
      });

      // Trigger precaching when on home page (after SW is ready, only once per session)
      if (window.location.pathname === '/' && !precacheTriggered) {
        navigator.serviceWorker.ready.then(function() {
          if (!precacheTriggered) {
            precacheTriggered = true;
            // Small delay to let SW fully initialize
            setTimeout(triggerPrecache, 1000);
          }
        });
      }
    }).catch(function(error) {
      console.error('[SW Register] Registration failed:', error);
    });

    // Handle controller change (new SW took over)
    navigator.serviceWorker.addEventListener('controllerchange', function() {
      console.log('[SW Register] Controller changed');
      // Trigger precache after controller change if on home page (only once per session)
      if (window.location.pathname === '/' && !precacheTriggered) {
        precacheTriggered = true;
        setTimeout(triggerPrecache, 500);
      }
    });
  }

  /**
   * Public API for service worker operations
   */
  window.ServiceWorkerAPI = {
    /**
     * Force check for service worker update
     */
    checkForUpdate: function() {
      if (registration) {
        return registration.update().then(function() {
          console.log('[SW API] Update check complete');
        });
      }
      return Promise.resolve();
    },

    /**
     * Clear all app caches
     * Returns a promise that resolves when caches are cleared
     */
    clearCaches: function() {
      if (navigator.serviceWorker.controller) {
        return new Promise(function(resolve, reject) {
          var messageChannel = new MessageChannel();
          messageChannel.port1.onmessage = function(event) {
            if (event.data.success) {
              console.log('[SW API] Caches cleared');
              resolve();
            } else {
              reject(new Error('Failed to clear caches'));
            }
          };
          navigator.serviceWorker.controller.postMessage(
            { type: 'CLEAR_CACHES' },
            [messageChannel.port2]
          );
        });
      }
      // Fallback: clear caches directly from this context
      return caches.keys().then(function(names) {
        return Promise.all(
          names.filter(function(name) {
            return name.startsWith('kr-');
          }).map(function(name) {
            return caches.delete(name);
          })
        );
      }).then(function() {
        console.log('[SW API] Caches cleared (direct)');
      });
    },

    /**
     * Get registration object
     */
    getRegistration: function() {
      return registration;
    },

    /**
     * Check if service worker is active
     */
    isActive: function() {
      return !!(registration && registration.active);
    },

    /**
     * Check if update is available
     */
    hasUpdate: function() {
      return !!(registration && registration.waiting);
    },

    /**
     * Apply waiting update and reload
     */
    applyUpdate: function() {
      if (registration && registration.waiting) {
        registration.waiting.postMessage({ type: 'SKIP_WAITING' });
        window.location.reload();
      }
    },

    /**
     * Get list of cached page paths
     * Returns a promise that resolves with an array of pathname strings
     */
    getCachedPages: function() {
      if (navigator.serviceWorker.controller) {
        return new Promise(function(resolve) {
          var messageChannel = new MessageChannel();
          messageChannel.port1.onmessage = function(event) {
            resolve(event.data.cachedPages || []);
          };
          navigator.serviceWorker.controller.postMessage(
            { type: 'GET_CACHED_PAGES' },
            [messageChannel.port2]
          );
        });
      }
      // Fallback: check cache directly (dynamically find pages cache)
      return caches.keys().then(function(names) {
        var pagesCache = names.find(function(name) {
          return name.startsWith('kr-pages-');
        });
        if (!pagesCache) {
          return [];
        }
        return caches.open(pagesCache).then(function(cache) {
          return cache.keys();
        }).then(function(keys) {
          return keys.map(function(request) {
            return new URL(request.url).pathname;
          });
        });
      }).catch(function() {
        return [];
      });
    },

    /**
     * Check if a specific page is cached
     */
    isPageCached: function(pathname) {
      return this.getCachedPages().then(function(pages) {
        return pages.includes(pathname);
      });
    },

    /**
     * Manually trigger precaching of reference pages
     */
    precachePages: function() {
      if (navigator.serviceWorker.controller) {
        navigator.serviceWorker.controller.postMessage({ type: 'PRECACHE_PAGES' });
        return Promise.resolve(true);
      }
      return Promise.resolve(false);
    }
  };

  // Register on page load
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', registerServiceWorker);
  } else {
    registerServiceWorker();
  }
})();

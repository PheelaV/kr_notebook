/**
 * Service Worker for Korean Hangul Learning App
 *
 * Caching strategy:
 * - Network-only: Auth routes, POST endpoints, mutations
 * - Cache-first when offline: Reference and Library pages (instant response)
 * - Cache-first: Static assets (CSS, JS, images)
 * - Precache on demand: Reference pages cached when user visits home
 */

'use strict';

// Fetch timeout in milliseconds (10 seconds)
const FETCH_TIMEOUT_MS = 10000;

/**
 * Fetch with timeout - prevents hanging requests from blocking the page.
 * WebKit can hang on some requests that never resolve/reject.
 * @param {Request|string} request - The request to fetch
 * @param {Object} options - Fetch options
 * @returns {Promise<Response>}
 */
function fetchWithTimeout(request, options) {
  return new Promise(function(resolve, reject) {
    var timeoutId = setTimeout(function() {
      reject(new Error('Fetch timeout'));
    }, FETCH_TIMEOUT_MS);

    fetch(request, options).then(function(response) {
      clearTimeout(timeoutId);
      resolve(response);
    }).catch(function(error) {
      clearTimeout(timeoutId);
      reject(error);
    });
  });
}

// Bump version to trigger update
const CACHE_VERSION = '30';
const CACHE_NAMES = {
  static: `kr-static-${CACHE_VERSION}`,
  pages: `kr-pages-${CACHE_VERSION}`,
  cdn: `kr-cdn-${CACHE_VERSION}`
};

// Static assets to precache on install
const PRECACHE_STATIC = [
  '/static/css/styles.css',
  '/static/js/card-interactions.js',
  '/static/js/auth.js',
  '/static/js/sw-register.js',
  '/static/js/offline-detect.js',
  '/static/js/offline-storage.js',
  '/static/js/offline-study.js',
  '/static/js/offline-sync.js',
  '/static/js/vocabulary-search.js',
  '/static/wasm/offline_srs.js',
  '/static/wasm/offline_srs_bg.wasm',
  '/static/favicon.svg',
  '/static/favicon.ico',
  '/static/favicon-16x16.png',
  '/static/favicon-32x32.png',
  '/static/apple-touch-icon.png',
  '/static/android-chrome-192x192.png',
  '/static/android-chrome-512x512.png',
  '/static/site.webmanifest',
  '/offline',
  '/offline-study'
];

// CDN resources to precache
const PRECACHE_CDN = [
  'https://unpkg.com/htmx.org@2.0.4/dist/htmx.min.js',
  'https://code.iconify.design/iconify-icon/2.1.0/iconify-icon.min.js',
  'https://cdn.jsdelivr.net/npm/fuse.js@7.0.0/dist/fuse.min.js'
];

// Fallback pages to precache if dynamic fetch fails (static pages only)
// Dynamic pack/lesson URLs are fetched from /api/precache-urls
const PRECACHE_PAGES_FALLBACK = [
  '/settings',
  '/reference',
  '/reference/basics',
  '/reference/tier1',
  '/reference/tier2',
  '/reference/tier3',
  '/reference/tier4',
  '/library',
  '/library/characters',
  '/library/vocabulary',
  '/guide'
];

// Routes that should never be cached (network-only)
const NETWORK_ONLY_PATTERNS = [
  /^\/login/,
  /^\/register/,
  /^\/logout/,
  /^\/guest/,
  /^\/api\//,
  /^\/review/,
  /^\/validate-answer/,
  /^\/next-card/,
  /^\/unlock-tier/,
  /^\/diagnostic/,
  /^\/settings\//,
  /^\/study\/filter/,
  /^\/practice-next/,
  /^\/practice-validate/,
  /^\/listen\/answer/,
  /^\/listen\/skip/
];

// Routes to cache with offline-first strategy
const OFFLINE_FIRST_PATTERNS = [
  /^\/settings$/,
  /^\/reference/,
  /^\/library/,
  /^\/guide$/
];

// Routes to cache with cache-first strategy
const CACHE_FIRST_PATTERNS = [
  /^\/static\//,
  /^\/audio\/scraped\//
];

// Offline study route - redirect to offline study page when offline
const OFFLINE_STUDY_PATTERN = /^\/study$/;

/**
 * Install event - precache static assets and CDN resources
 */
self.addEventListener('install', function(event) {
  console.log('[SW] Installing version', CACHE_VERSION);
  event.waitUntil(
    Promise.all([
      // Precache static assets (bypass browser cache to get fresh files)
      caches.open(CACHE_NAMES.static).then(function(cache) {
        return Promise.all(
          PRECACHE_STATIC.map(function(url) {
            return fetchWithTimeout(url, { cache: 'reload' }).then(function(response) {
              if (response.ok) {
                return cache.put(url, response);
              }
            }).catch(function(error) {
              console.warn('[SW] Failed to precache:', url, error.message);
            });
          })
        );
      }),
      // Precache CDN resources (may fail if offline during install, with timeout)
      caches.open(CACHE_NAMES.cdn).then(function(cache) {
        return Promise.all(
          PRECACHE_CDN.map(function(url) {
            return fetchWithTimeout(url).then(function(response) {
              if (response.ok) {
                return cache.put(url, response);
              }
            }).catch(function(error) {
              console.warn('[SW] Failed to precache CDN resource:', url, error.message);
            });
          })
        );
      })
    ]).then(function() {
      console.log('[SW] Precaching complete, skipping waiting');
      return self.skipWaiting();
    })
  );
});

/**
 * Activate event - clean up old caches
 */
self.addEventListener('activate', function(event) {
  console.log('[SW] Activating version', CACHE_VERSION);
  event.waitUntil(
    caches.keys().then(function(cacheNames) {
      return Promise.all(
        cacheNames
          .filter(function(name) {
            // Delete caches that start with our prefix but aren't current version
            return name.startsWith('kr-') &&
                   !Object.values(CACHE_NAMES).includes(name);
          })
          .map(function(name) {
            console.log('[SW] Deleting old cache:', name);
            return caches.delete(name);
          })
      );
    }).then(function() {
      console.log('[SW] Taking control of clients');
      return self.clients.claim();
    })
  );
});

/**
 * Cache-first strategy (for static assets)
 * Returns cached response if available, otherwise fetches and caches
 * Uses ignoreSearch to match URLs with different query strings (versioning)
 */
function cacheFirst(request, cacheName) {
  // For static assets, ignore query string when matching cache
  // This allows versioned URLs (?v=abc) to match non-versioned cached files
  var matchOptions = cacheName === CACHE_NAMES.static ? { ignoreSearch: true } : {};

  return caches.match(request, matchOptions).then(function(cachedResponse) {
    if (cachedResponse) {
      return cachedResponse;
    }

    return fetchWithTimeout(request).then(function(networkResponse) {
      if (networkResponse.ok) {
        var responseToCache = networkResponse.clone();
        caches.open(cacheName).then(function(cache) {
          // Store with original URL (including query string) for future exact matches
          cache.put(request, responseToCache);
        });
      }
      return networkResponse;
    }).catch(function(error) {
      // Network failed, timed out, or not in cache - return empty response for CDN resources
      // (prevents console errors when offline or when fetch hangs)
      console.warn('[SW] Cache-first fetch failed:', request.url, error.message);
      return new Response('', { status: 503, statusText: 'Service Unavailable (offline)' });
    });
  });
}

/**
 * Stale-while-revalidate strategy (for online mode)
 * Returns cached response immediately, fetches update in background
 */
function staleWhileRevalidate(request, cacheName) {
  return caches.open(cacheName).then(function(cache) {
    return cache.match(request).then(function(cachedResponse) {
      // Start network fetch in background (with timeout to prevent hanging)
      var fetchPromise = fetchWithTimeout(request).then(function(networkResponse) {
        if (networkResponse.ok) {
          cache.put(request, networkResponse.clone());
        }
        return networkResponse;
      }).catch(function(error) {
        console.warn('[SW] Background fetch failed:', error.message);
        return null;
      });

      // Return cached response immediately if available
      if (cachedResponse) {
        return cachedResponse;
      }

      // No cache - wait for network
      return fetchPromise.then(function(networkResponse) {
        if (networkResponse) {
          return networkResponse;
        }
        // Network failed - serve offline page as fallback
        // (navigator.onLine is unreliable, especially with DevTools offline)
        return caches.match('/offline');
      });
    });
  });
}

/**
 * Cache-first when offline (for reference/library pages)
 * When offline: immediately return cache or offline page (no network wait)
 * When online: use stale-while-revalidate
 */
function cacheFirstOffline(request, cacheName) {
  // If offline, go straight to cache - no network wait
  if (!navigator.onLine) {
    return caches.match(request).then(function(cachedResponse) {
      if (cachedResponse) {
        return cachedResponse;
      }
      return caches.match('/offline');
    });
  }

  // Online: use stale-while-revalidate for fresh content
  return staleWhileRevalidate(request, cacheName);
}

/**
 * Handle home page requests
 * When offline: serve offline page immediately
 * When online: network-first with cache fallback
 */
function handleHomePage(request) {
  // If offline, immediately serve offline page
  if (!navigator.onLine) {
    return caches.match('/offline');
  }

  // Online: try network first (with timeout to prevent hanging)
  return fetchWithTimeout(request).then(function(networkResponse) {
    // Cache the home page for faster future loads
    if (networkResponse.ok) {
      var responseToCache = networkResponse.clone();
      caches.open(CACHE_NAMES.pages).then(function(cache) {
        cache.put(request, responseToCache);
      });
    }
    return networkResponse;
  }).catch(function(error) {
    console.warn('[SW] Home page fetch failed:', error.message);
    // Try cache, then offline page
    return caches.match(request).then(function(cached) {
      return cached || caches.match('/offline');
    });
  });
}

/**
 * Network-first with offline fallback (for other navigation requests)
 */
function networkFirstWithFallback(request) {
  // If offline, immediately check cache
  if (!navigator.onLine) {
    return caches.match(request).then(function(cached) {
      return cached || caches.match('/offline');
    });
  }

  return fetchWithTimeout(request).then(function(networkResponse) {
    return networkResponse;
  }).catch(function(error) {
    console.warn('[SW] Network request failed:', error.message);
    // Network failed or timed out - try cache, then offline page
    // (navigator.onLine is unreliable, especially with DevTools offline)
    return caches.match(request).then(function(cachedResponse) {
      return cachedResponse || caches.match('/offline');
    });
  });
}

/**
 * Check if URL matches any pattern in the list
 */
function matchesPattern(pathname, patterns) {
  return patterns.some(function(pattern) {
    return pattern.test(pathname);
  });
}

/**
 * Fetch event - route requests to appropriate caching strategy
 */
self.addEventListener('fetch', function(event) {
  var url = new URL(event.request.url);

  // Skip non-GET requests (POST, DELETE, etc.)
  if (event.request.method !== 'GET') {
    return;
  }

  // Network-only for auth and mutation endpoints
  if (matchesPattern(url.pathname, NETWORK_ONLY_PATTERNS)) {
    return;
  }

  // Cache-first for static assets (same origin)
  if (matchesPattern(url.pathname, CACHE_FIRST_PATTERNS)) {
    event.respondWith(cacheFirst(event.request, CACHE_NAMES.static));
    return;
  }

  // Cache-first for CDN resources (different origin)
  if (url.origin !== location.origin) {
    event.respondWith(cacheFirst(event.request, CACHE_NAMES.cdn));
    return;
  }

  // Home page gets special handling
  if (url.pathname === '/') {
    event.respondWith(handleHomePage(event.request));
    return;
  }

  // Study page when offline - redirect to offline study page
  if (OFFLINE_STUDY_PATTERN.test(url.pathname) && !navigator.onLine) {
    event.respondWith(
      caches.match('/offline-study').then(function(cachedResponse) {
        if (cachedResponse) {
          return cachedResponse;
        }
        // Fallback to network if not cached (with timeout)
        return fetchWithTimeout(event.request).catch(function() {
          return caches.match('/offline');
        });
      })
    );
    return;
  }

  // Cache-first when offline for reference/library pages
  if (matchesPattern(url.pathname, OFFLINE_FIRST_PATTERNS)) {
    event.respondWith(cacheFirstOffline(event.request, CACHE_NAMES.pages));
    return;
  }

  // Network-first with offline fallback for other navigation requests
  if (event.request.mode === 'navigate') {
    event.respondWith(networkFirstWithFallback(event.request));
    return;
  }

  // Default: let browser handle the request normally
});

/**
 * Message event - handle messages from clients
 */
self.addEventListener('message', function(event) {
  if (event.data && event.data.type === 'SKIP_WAITING') {
    self.skipWaiting();
  }

  // Precache reference pages (triggered from home page)
  // Fetches dynamic URLs from server, falls back to static list
  if (event.data && event.data.type === 'PRECACHE_PAGES') {
    console.log('[SW] Precaching pages...');
    event.waitUntil(
      // Try to get dynamic URLs from server first (with timeout)
      fetchWithTimeout('/api/precache-urls')
        .then(function(response) {
          if (response.ok) {
            return response.json();
          }
          throw new Error('Failed to fetch precache URLs');
        })
        .catch(function(error) {
          console.warn('[SW] Using fallback URLs:', error);
          return PRECACHE_PAGES_FALLBACK;
        })
        .then(function(urls) {
          console.log('[SW] Precaching', urls.length, 'URLs');
          return caches.open(CACHE_NAMES.pages).then(function(cache) {
            return Promise.all(
              urls.map(function(url) {
                return fetchWithTimeout(url).then(function(response) {
                  if (response.ok) {
                    console.log('[SW] Cached:', url);
                    return cache.put(url, response);
                  }
                }).catch(function(error) {
                  console.warn('[SW] Failed to precache:', url, error.message);
                });
              })
            );
          });
        })
        .then(function() {
          console.log('[SW] Pages precached');
        })
    );
  }

  // Clear all caches
  if (event.data && event.data.type === 'CLEAR_CACHES') {
    event.waitUntil(
      caches.keys().then(function(names) {
        return Promise.all(
          names.filter(function(name) {
            return name.startsWith('kr-');
          }).map(function(name) {
            return caches.delete(name);
          })
        );
      }).then(function() {
        event.ports[0].postMessage({ success: true });
      })
    );
  }

  // Get list of cached pages (for menu awareness)
  if (event.data && event.data.type === 'GET_CACHED_PAGES') {
    caches.open(CACHE_NAMES.pages).then(function(cache) {
      return cache.keys();
    }).then(function(keys) {
      var paths = keys.map(function(request) {
        return new URL(request.url).pathname;
      });
      event.ports[0].postMessage({ cachedPages: paths });
    });
  }
});

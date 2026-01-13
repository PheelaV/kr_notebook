/**
 * IndexedDB storage module for offline study sessions.
 *
 * Provides persistent storage for:
 * - Downloaded study sessions (cards + settings)
 * - Review responses made during offline study
 */

'use strict';

const OfflineStorage = (function() {
  const DB_NAME = 'kr-offline-study';
  const DB_VERSION = 1;

  // Store names
  const STORES = {
    SESSIONS: 'sessions',
    RESPONSES: 'responses'
  };

  let db = null;

  /**
   * Open the IndexedDB database.
   * @returns {Promise<IDBDatabase>}
   */
  function openDatabase() {
    if (db) {
      return Promise.resolve(db);
    }

    return new Promise(function(resolve, reject) {
      const request = indexedDB.open(DB_NAME, DB_VERSION);

      request.onerror = function(event) {
        console.error('[OfflineStorage] Database error:', event.target.error);
        reject(event.target.error);
      };

      request.onsuccess = function(event) {
        db = event.target.result;
        console.log('[OfflineStorage] Database opened');
        resolve(db);
      };

      request.onupgradeneeded = function(event) {
        const database = event.target.result;

        // Sessions store: one active session at a time
        if (!database.objectStoreNames.contains(STORES.SESSIONS)) {
          const sessionStore = database.createObjectStore(STORES.SESSIONS, {
            keyPath: 'session_id'
          });
          sessionStore.createIndex('created_at', 'created_at', { unique: false });
        }

        // Responses store: reviews made during offline study
        if (!database.objectStoreNames.contains(STORES.RESPONSES)) {
          const responseStore = database.createObjectStore(STORES.RESPONSES, {
            keyPath: 'id',
            autoIncrement: true
          });
          responseStore.createIndex('session_id', 'session_id', { unique: false });
          responseStore.createIndex('card_id', 'card_id', { unique: false });
          responseStore.createIndex('timestamp', 'timestamp', { unique: false });
        }

        console.log('[OfflineStorage] Database schema created/upgraded');
      };
    });
  }

  /**
   * Save a downloaded session to IndexedDB.
   * Clears any existing session first (one session at a time).
   * @param {Object} session - Session data from /api/study/download-session
   * @returns {Promise<void>}
   */
  async function saveSession(session) {
    const database = await openDatabase();

    return new Promise(function(resolve, reject) {
      const transaction = database.transaction([STORES.SESSIONS, STORES.RESPONSES], 'readwrite');

      transaction.onerror = function(event) {
        reject(event.target.error);
      };

      transaction.oncomplete = function() {
        console.log('[OfflineStorage] Session saved:', session.session_id);
        resolve();
      };

      // Clear existing sessions
      const sessionStore = transaction.objectStore(STORES.SESSIONS);
      sessionStore.clear();

      // Clear existing responses
      const responseStore = transaction.objectStore(STORES.RESPONSES);
      responseStore.clear();

      // Add new session
      sessionStore.add(session);
    });
  }

  /**
   * Get the current offline session.
   * @returns {Promise<Object|null>} The session or null if none exists
   */
  async function getSession() {
    const database = await openDatabase();

    return new Promise(function(resolve, reject) {
      const transaction = database.transaction([STORES.SESSIONS], 'readonly');
      const store = transaction.objectStore(STORES.SESSIONS);
      const request = store.getAll();

      request.onerror = function(event) {
        reject(event.target.error);
      };

      request.onsuccess = function(event) {
        const sessions = event.target.result;
        if (sessions && sessions.length > 0) {
          resolve(sessions[0]);
        } else {
          resolve(null);
        }
      };
    });
  }

  /**
   * Check if an offline session exists.
   * @returns {Promise<boolean>}
   */
  async function hasSession() {
    const session = await getSession();
    return session !== null;
  }

  /**
   * Get session age in hours.
   * @returns {Promise<number|null>} Hours since session was created, or null if no session
   */
  async function getSessionAgeHours() {
    const session = await getSession();
    if (!session) {
      return null;
    }

    const created = new Date(session.created_at);
    const now = new Date();
    const diffMs = now - created;
    return diffMs / (1000 * 60 * 60);
  }

  /**
   * Add a review response during offline study.
   * @param {Object} response - Review response data
   * @returns {Promise<void>}
   */
  async function addResponse(response) {
    const database = await openDatabase();

    return new Promise(function(resolve, reject) {
      const transaction = database.transaction([STORES.RESPONSES], 'readwrite');
      const store = transaction.objectStore(STORES.RESPONSES);

      const record = {
        ...response,
        timestamp: response.timestamp || new Date().toISOString()
      };

      const request = store.add(record);

      request.onerror = function(event) {
        reject(event.target.error);
      };

      request.onsuccess = function() {
        console.log('[OfflineStorage] Response added for card:', response.card_id);
        resolve();
      };
    });
  }

  /**
   * Get all responses for the current session.
   * @returns {Promise<Array>} Array of response objects
   */
  async function getResponses() {
    const database = await openDatabase();

    return new Promise(function(resolve, reject) {
      const transaction = database.transaction([STORES.RESPONSES], 'readonly');
      const store = transaction.objectStore(STORES.RESPONSES);
      const request = store.getAll();

      request.onerror = function(event) {
        reject(event.target.error);
      };

      request.onsuccess = function(event) {
        resolve(event.target.result || []);
      };
    });
  }

  /**
   * Get count of pending responses.
   * @returns {Promise<number>}
   */
  async function getPendingCount() {
    const responses = await getResponses();
    return responses.length;
  }

  /**
   * Clear all offline data (session and responses).
   * @returns {Promise<void>}
   */
  async function clearAll() {
    const database = await openDatabase();

    return new Promise(function(resolve, reject) {
      const transaction = database.transaction([STORES.SESSIONS, STORES.RESPONSES], 'readwrite');

      transaction.onerror = function(event) {
        reject(event.target.error);
      };

      transaction.oncomplete = function() {
        console.log('[OfflineStorage] All offline data cleared');
        resolve();
      };

      transaction.objectStore(STORES.SESSIONS).clear();
      transaction.objectStore(STORES.RESPONSES).clear();
    });
  }

  /**
   * Clear responses except for specified failed card IDs.
   * Used when sync partially succeeds.
   * @param {Array<number>} failedCardIds - Card IDs whose responses should be kept
   * @returns {Promise<void>}
   */
  async function clearSyncedResponses(failedCardIds) {
    if (!failedCardIds || failedCardIds.length === 0) {
      // No failures - clear everything
      return clearAll();
    }

    const database = await openDatabase();
    const responses = await getResponses();

    // Filter to keep only responses for failed cards
    const responsesToKeep = responses.filter(function(r) {
      return failedCardIds.includes(r.card_id);
    });

    return new Promise(function(resolve, reject) {
      const transaction = database.transaction([STORES.RESPONSES], 'readwrite');
      const store = transaction.objectStore(STORES.RESPONSES);

      transaction.onerror = function(event) {
        reject(event.target.error);
      };

      transaction.oncomplete = function() {
        console.log('[OfflineStorage] Cleared synced responses, kept', responsesToKeep.length, 'failed');
        resolve();
      };

      // Clear all responses first
      store.clear();

      // Re-add only the failed ones
      responsesToKeep.forEach(function(response) {
        // Remove the auto-generated id so it gets a new one
        var toAdd = { ...response };
        delete toAdd.id;
        store.add(toAdd);
      });
    });
  }

  /**
   * Update a card's SRS state in the session.
   * Used after each review to track updated state for next review.
   * @param {number} cardId - The card ID
   * @param {Object} newState - Updated SRS state
   * @returns {Promise<void>}
   */
  async function updateCardState(cardId, newState) {
    const database = await openDatabase();
    const session = await getSession();

    if (!session) {
      return;
    }

    // Find and update the card
    const cardIndex = session.cards.findIndex(function(c) {
      return c.card_id === cardId;
    });

    if (cardIndex === -1) {
      return;
    }

    // Update card state
    session.cards[cardIndex] = {
      ...session.cards[cardIndex],
      learning_step: newState.learning_step,
      fsrs_stability: newState.fsrs_stability,
      fsrs_difficulty: newState.fsrs_difficulty,
      next_review: newState.next_review
    };

    return new Promise(function(resolve, reject) {
      const transaction = database.transaction([STORES.SESSIONS], 'readwrite');
      const store = transaction.objectStore(STORES.SESSIONS);

      const request = store.put(session);

      request.onerror = function(event) {
        reject(event.target.error);
      };

      request.onsuccess = function() {
        resolve();
      };
    });
  }

  /**
   * Prepare sync payload from stored responses.
   * @returns {Promise<Object>} Sync request payload
   */
  async function prepareSyncPayload() {
    const session = await getSession();
    const responses = await getResponses();

    if (!session) {
      throw new Error('No session to sync');
    }

    return {
      session_id: session.session_id,
      reviews: responses.map(function(r) {
        return {
          card_id: r.card_id,
          quality: r.quality,
          is_correct: r.is_correct,
          hints_used: r.hints_used || 0,
          timestamp: r.timestamp,
          // Default to 0 for old responses that don't have these fields
          learning_step: r.learning_step !== undefined ? r.learning_step : 0,
          fsrs_stability: r.fsrs_stability,
          fsrs_difficulty: r.fsrs_difficulty,
          next_review: r.next_review || new Date().toISOString()
        };
      })
    };
  }

  // Public API
  return {
    openDatabase: openDatabase,
    saveSession: saveSession,
    getSession: getSession,
    hasSession: hasSession,
    getSessionAgeHours: getSessionAgeHours,
    addResponse: addResponse,
    getResponses: getResponses,
    getPendingCount: getPendingCount,
    clearAll: clearAll,
    clearSyncedResponses: clearSyncedResponses,
    updateCardState: updateCardState,
    prepareSyncPayload: prepareSyncPayload
  };
})();

// Export for use in other modules
window.OfflineStorage = OfflineStorage;

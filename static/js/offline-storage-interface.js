/**
 * Offline Storage Interface
 *
 * Defines the contract for storage operations. The real implementation uses IndexedDB,
 * but tests can inject a mock implementation.
 *
 * This follows the same pattern as Rust's trait-based testing - abstracting external
 * dependencies behind interfaces that can be swapped for testing.
 */

'use strict';

/**
 * In-memory storage implementation for testing.
 * Mimics IndexedDB behavior without browser dependencies.
 */
export function createMockStorage() {
  let session = null;
  let responses = [];
  let sessionCreatedAt = null;

  return {
    getSession: async function() {
      return session;
    },

    saveSession: async function(newSession) {
      session = { ...newSession };
      sessionCreatedAt = new Date();
    },

    hasSession: async function() {
      return session !== null && session.cards && session.cards.length > 0;
    },

    getSessionAgeHours: async function() {
      if (!sessionCreatedAt) return null;
      return (Date.now() - sessionCreatedAt.getTime()) / (1000 * 60 * 60);
    },

    addResponse: async function(response) {
      const existingIdx = responses.findIndex(r => r.card_id === response.card_id);
      if (existingIdx >= 0) {
        responses[existingIdx] = { ...response, id: responses[existingIdx].id };
      } else {
        responses.push({ ...response, id: responses.length + 1 });
      }
    },

    getResponses: async function() {
      return [...responses];
    },

    clearResponses: async function() {
      responses = [];
    },

    getPendingCount: async function() {
      return responses.length;
    },

    updateCardState: async function(cardId, newState) {
      if (!session || !session.cards) return;
      const card = session.cards.find(c => c.card_id === cardId);
      if (card) {
        Object.assign(card, newState);
      }
    },

    clearAll: async function() {
      session = null;
      responses = [];
      sessionCreatedAt = null;
    },

    // Test helpers
    _getResponses: function() { return responses; },
    _setSession: function(s) { session = s; sessionCreatedAt = new Date(); }
  };
}

/**
 * Mock WASM module for testing.
 * Provides deterministic validation and SRS calculations.
 */
export function createMockWasm() {
  return {
    validate_answer: function(userAnswer, correctAnswer, usedHint) {
      const isExactMatch = userAnswer.toLowerCase().trim() === correctAnswer.toLowerCase().trim();
      const isClose = userAnswer.length > 0 &&
                      correctAnswer.toLowerCase().includes(userAnswer.toLowerCase().trim());

      let result, quality, is_correct;
      if (isExactMatch) {
        result = 'Correct';
        quality = usedHint ? 3 : 4;
        is_correct = true;
      } else if (isClose) {
        result = 'CloseEnough';
        quality = 3;
        is_correct = true;
      } else {
        result = 'Wrong';
        quality = 0;
        is_correct = false;
      }

      return JSON.stringify({ result, quality, is_correct });
    },

    calculate_next_review: function(cardStateJson, quality, desiredRetention, focusMode) {
      const cardState = JSON.parse(cardStateJson);
      const isCorrect = quality >= 3;

      let newStep = cardState.learning_step;
      let newStability = cardState.fsrs_stability || 1.0;
      let newDifficulty = cardState.fsrs_difficulty || 5.0;

      if (isCorrect) {
        newStep = Math.min(newStep + 1, 4);
        newStability *= 1.5;
      } else {
        newStep = 0;
        newStability *= 0.5;
        newDifficulty = Math.min(newDifficulty + 0.5, 10);
      }

      const nextReview = new Date(Date.now() + newStability * 24 * 60 * 60 * 1000).toISOString();

      return JSON.stringify({
        learning_step: newStep,
        fsrs_stability: newStability,
        fsrs_difficulty: newDifficulty,
        next_review: nextReview
      });
    },

    get_hint: function(answer, hintLevel) {
      if (hintLevel === 1) return answer.charAt(0) + '...';
      if (hintLevel === 2) return answer.substring(0, Math.ceil(answer.length / 2)) + '...';
      return answer;
    }
  };
}

/**
 * Mock connectivity detector for testing.
 */
export function createMockConnectivity() {
  let isOnline = true;
  const listeners = { online: [], offline: [] };

  return {
    isOnline: function() { return isOnline; },

    addEventListener: function(event, callback) {
      if (listeners[event]) {
        listeners[event].push(callback);
      }
    },

    removeEventListener: function(event, callback) {
      if (listeners[event]) {
        listeners[event] = listeners[event].filter(cb => cb !== callback);
      }
    },

    simulateOnline: function() {
      isOnline = true;
      listeners.online.forEach(cb => cb());
    },

    simulateOffline: function() {
      isOnline = false;
      listeners.offline.forEach(cb => cb());
    }
  };
}

// Browser global exports (for non-module usage)
if (typeof window !== 'undefined') {
  window.createMockStorage = createMockStorage;
  window.createMockWasm = createMockWasm;
  window.createMockConnectivity = createMockConnectivity;
}

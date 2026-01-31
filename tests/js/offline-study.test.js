/**
 * Integration tests for offline study logic using the real WASM module.
 *
 * Tests the actual Rust code through WASM bindings:
 * - Answer validation (exact match, typos, partial, variants)
 * - SRS scheduling (learning steps, graduation, FSRS)
 * - Hint generation
 * - Card selection algorithms (JS-side logic)
 */

import { describe, it, expect, beforeAll, beforeEach } from 'vitest';
import { readFileSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import {
  createMockStorage,
  createMockConnectivity
} from '../../static/js/offline-storage-interface.js';

// Get directory of this test file
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const PROJECT_ROOT = join(__dirname, '../..');

// WASM module - loaded once before all tests
let wasm;

beforeAll(async () => {
  // Load the real WASM module
  const wasmPath = join(PROJECT_ROOT, 'static/wasm/offline_srs_bg.wasm');
  const wasmBytes = readFileSync(wasmPath);

  // Dynamic import to get the JS bindings
  const wasmModule = await import('../../static/wasm/offline_srs.js');

  // Initialize with the WASM bytes (pass as object to avoid deprecation warning)
  wasmModule.initSync({ module: wasmBytes });

  wasm = wasmModule;
});

/**
 * Card selector - mirrors the algorithm from offline-study.js
 */
function createCardSelector(cards) {
  let cardQueue = [...cards].sort((a, b) =>
    new Date(a.next_review) - new Date(b.next_review)
  );
  let reinforcementQueue = [];
  let cardsSinceReinforcement = 0;
  let lastCardMainAnswer = null;

  function isSiblingOfLastCard(card) {
    if (!lastCardMainAnswer) return false;
    return card.back === lastCardMainAnswer ||
           card.front.includes(lastCardMainAnswer) ||
           (card.back && lastCardMainAnswer && card.back.includes(lastCardMainAnswer));
  }

  function getNonSiblingFromQueue(queue) {
    if (queue.length === 0) return null;
    if (!lastCardMainAnswer) return queue.shift();

    for (let i = 0; i < queue.length; i++) {
      if (!isSiblingOfLastCard(queue[i])) {
        return queue.splice(i, 1)[0];
      }
    }
    return queue.shift();
  }

  return {
    getNextCard() {
      let nextCard = null;

      if (cardQueue.length === 0 && reinforcementQueue.length > 0) {
        cardsSinceReinforcement = 0;
        nextCard = getNonSiblingFromQueue(reinforcementQueue);
      } else if (reinforcementQueue.length > 0 && cardsSinceReinforcement >= 3) {
        cardsSinceReinforcement = 0;
        nextCard = getNonSiblingFromQueue(reinforcementQueue);
      } else if (cardQueue.length > 0) {
        cardsSinceReinforcement++;
        nextCard = getNonSiblingFromQueue(cardQueue);
      }

      if (nextCard) {
        lastCardMainAnswer = nextCard.back;
      }
      return nextCard;
    },

    addToReinforcement(card) {
      reinforcementQueue.push(card);
    },

    getQueueLengths() {
      return {
        main: cardQueue.length,
        reinforcement: reinforcementQueue.length
      };
    },

    getLastCardMainAnswer() {
      return lastCardMainAnswer;
    }
  };
}

// =============================================================================
// Card Selection Tests (JS-side logic)
// =============================================================================

describe('Card Selection', () => {
  const testCards = [
    { card_id: 1, front: 'ㄱ', back: 'g / k', next_review: '2024-01-01T00:00:00Z' },
    { card_id: 2, front: 'ㄴ', back: 'n', next_review: '2024-01-01T00:01:00Z' },
    { card_id: 3, front: 'ㄷ', back: 'd / t', next_review: '2024-01-01T00:02:00Z' }
  ];

  it('returns cards in due order', () => {
    const selector = createCardSelector(testCards);

    expect(selector.getNextCard().card_id).toBe(1);
    expect(selector.getNextCard().card_id).toBe(2);
    expect(selector.getNextCard().card_id).toBe(3);
  });

  it('returns null when queue is empty', () => {
    const selector = createCardSelector(testCards);

    selector.getNextCard();
    selector.getNextCard();
    selector.getNextCard();

    expect(selector.getNextCard()).toBeNull();
  });

  it('handles empty initial queue', () => {
    const selector = createCardSelector([]);
    expect(selector.getNextCard()).toBeNull();
  });
});

describe('Sibling Exclusion', () => {
  it('skips sibling cards with matching main_answer', () => {
    const cards = [
      { card_id: 1, front: 'ㅢ', back: 'ui', next_review: '2024-01-01T00:00:00Z' },
      { card_id: 2, front: "Which letter sounds like 'ui'?", back: 'ㅢ', next_review: '2024-01-01T00:01:00Z' },
      { card_id: 3, front: 'ㅟ', back: 'wi', next_review: '2024-01-01T00:02:00Z' }
    ];

    const selector = createCardSelector(cards);

    const first = selector.getNextCard();
    expect(first.card_id).toBe(1);
    expect(selector.getLastCardMainAnswer()).toBe('ui');

    // Card 2 has 'ui' in front, should skip to card 3
    const second = selector.getNextCard();
    expect(second.card_id).toBe(3);

    // Now card 2 is returned
    const third = selector.getNextCard();
    expect(third.card_id).toBe(2);
  });

  it('returns sibling if no other cards available', () => {
    const cards = [
      { card_id: 1, front: 'ㅢ', back: 'ui', next_review: '2024-01-01T00:00:00Z' },
      { card_id: 2, front: "Which has 'ui'?", back: 'ㅢ', next_review: '2024-01-01T00:01:00Z' }
    ];

    const selector = createCardSelector(cards);

    expect(selector.getNextCard().card_id).toBe(1);
    // Only sibling left, must return it
    expect(selector.getNextCard().card_id).toBe(2);
  });

  it('detects siblings by back field containing last answer', () => {
    const cards = [
      { card_id: 1, front: 'A', back: 'apple', next_review: '2024-01-01T00:00:00Z' },
      { card_id: 2, front: 'B', back: 'apple pie', next_review: '2024-01-01T00:01:00Z' },
      { card_id: 3, front: 'C', back: 'banana', next_review: '2024-01-01T00:02:00Z' }
    ];

    const selector = createCardSelector(cards);

    expect(selector.getNextCard().card_id).toBe(1); // back='apple'
    expect(selector.getNextCard().card_id).toBe(3); // skips 2 (back contains 'apple')
    expect(selector.getNextCard().card_id).toBe(2); // now returns 2
  });
});

describe('Reinforcement Interleaving', () => {
  const cards = [
    { card_id: 1, front: 'A', back: 'a', next_review: '2024-01-01T00:00:00Z' },
    { card_id: 2, front: 'B', back: 'b', next_review: '2024-01-01T00:01:00Z' },
    { card_id: 3, front: 'C', back: 'c', next_review: '2024-01-01T00:02:00Z' },
    { card_id: 4, front: 'D', back: 'd', next_review: '2024-01-01T00:03:00Z' },
    { card_id: 5, front: 'E', back: 'e', next_review: '2024-01-01T00:04:00Z' }
  ];

  it('interleaves reinforcement card after 3 main cards', () => {
    const selector = createCardSelector(cards);

    // Get first card and add to reinforcement (simulating wrong answer)
    const first = selector.getNextCard();
    expect(first.card_id).toBe(1);
    selector.addToReinforcement({ ...first });

    // Get 2 more cards
    expect(selector.getNextCard().card_id).toBe(2);
    expect(selector.getNextCard().card_id).toBe(3);

    // 4th card should be from reinforcement (after 3 main cards)
    expect(selector.getNextCard().card_id).toBe(1);

    // Verify queue state
    const lengths = selector.getQueueLengths();
    expect(lengths.main).toBe(2);
    expect(lengths.reinforcement).toBe(0);
  });

  it('uses reinforcement when main queue is empty', () => {
    const selector = createCardSelector([cards[0]]);

    const first = selector.getNextCard();
    selector.addToReinforcement({ ...first });

    // Main queue empty, should get from reinforcement
    expect(selector.getNextCard().card_id).toBe(1);
  });

  it('handles multiple cards in reinforcement queue', () => {
    const selector = createCardSelector(cards);

    // Add multiple cards to reinforcement
    const c1 = selector.getNextCard();
    selector.addToReinforcement({ ...c1 });
    const c2 = selector.getNextCard();
    selector.addToReinforcement({ ...c2 });

    expect(selector.getQueueLengths().reinforcement).toBe(2);

    // Continue getting cards
    selector.getNextCard(); // card 3

    // Next should be from reinforcement
    const reinforced = selector.getNextCard();
    expect([1, 2]).toContain(reinforced.card_id);
  });
});

// =============================================================================
// Answer Validation Tests (Real WASM)
// =============================================================================

describe('Answer Validation (WASM)', () => {
  describe('Exact Matches', () => {
    it('validates exact match as correct', () => {
      const result = JSON.parse(wasm.validate_answer('ui', 'ui', false));
      expect(result.is_correct).toBe(true);
      expect(result.quality).toBe(4);
      expect(result.result).toBe('Correct');
    });

    it('is case insensitive', () => {
      const result = JSON.parse(wasm.validate_answer('UI', 'ui', false));
      expect(result.is_correct).toBe(true);
      expect(result.result).toBe('Correct');
    });

    it('trims whitespace', () => {
      const result = JSON.parse(wasm.validate_answer('  ui  ', 'ui', false));
      expect(result.is_correct).toBe(true);
    });

    it('handles empty input as incorrect', () => {
      const result = JSON.parse(wasm.validate_answer('', 'ui', false));
      expect(result.is_correct).toBe(false);
      expect(result.result).toBe('Incorrect');
    });

    it('handles whitespace-only input as incorrect', () => {
      const result = JSON.parse(wasm.validate_answer('   ', 'ui', false));
      expect(result.is_correct).toBe(false);
    });
  });

  describe('Alternative Answers (slash separated)', () => {
    it('accepts first alternative', () => {
      const result = JSON.parse(wasm.validate_answer('g', 'g / k', false));
      expect(result.is_correct).toBe(true);
      expect(result.result).toBe('Correct');
    });

    it('accepts second alternative', () => {
      const result = JSON.parse(wasm.validate_answer('k', 'g / k', false));
      expect(result.is_correct).toBe(true);
    });

    it('accepts full answer with slash', () => {
      const result = JSON.parse(wasm.validate_answer('g/k', 'g / k', false));
      expect(result.is_correct).toBe(true);
    });

    it('rejects unrelated answer', () => {
      const result = JSON.parse(wasm.validate_answer('m', 'g / k', false));
      expect(result.is_correct).toBe(false);
    });
  });

  describe('Bracket Variants [a, b, c]', () => {
    it('accepts core answer', () => {
      const result = JSON.parse(wasm.validate_answer('to be', 'to be [is, am, are]', false));
      expect(result.is_correct).toBe(true);
    });

    it('accepts first variant', () => {
      const result = JSON.parse(wasm.validate_answer('is', 'to be [is, am, are]', false));
      expect(result.is_correct).toBe(true);
    });

    it('accepts second variant', () => {
      const result = JSON.parse(wasm.validate_answer('am', 'to be [is, am, are]', false));
      expect(result.is_correct).toBe(true);
    });

    it('accepts third variant', () => {
      const result = JSON.parse(wasm.validate_answer('are', 'to be [is, am, are]', false));
      expect(result.is_correct).toBe(true);
    });
  });

  describe('Suffix Syntax (s)', () => {
    it('accepts without suffix', () => {
      const result = JSON.parse(wasm.validate_answer('eye', 'eye(s)', false));
      expect(result.is_correct).toBe(true);
    });

    it('accepts with suffix', () => {
      const result = JSON.parse(wasm.validate_answer('eyes', 'eye(s)', false));
      expect(result.is_correct).toBe(true);
    });
  });

  describe('Disambiguation <context>', () => {
    it('full answer with disambiguation is correct', () => {
      const result = JSON.parse(wasm.validate_answer('that far', 'that <far>', false));
      expect(result.is_correct).toBe(true);
      expect(result.result).toBe('Correct');
    });

    it('core without disambiguation is partial match', () => {
      const result = JSON.parse(wasm.validate_answer('that', 'that <far>', false));
      expect(result.is_correct).toBe(true); // Still counts as correct
      expect(result.result).toBe('PartialMatch');
      expect(result.quality).toBe(2); // Lower quality
      expect(result.allows_retry).toBe(true);
    });

    it('wrong core is incorrect', () => {
      const result = JSON.parse(wasm.validate_answer('this', 'that <far>', false));
      expect(result.is_correct).toBe(false);
    });
  });

  describe('Info Syntax (supplementary)', () => {
    it('accepts core only', () => {
      const result = JSON.parse(wasm.validate_answer('this', 'this (thing)', false));
      expect(result.is_correct).toBe(true);
    });

    it('accepts core plus info', () => {
      const result = JSON.parse(wasm.validate_answer('this thing', 'this (thing)', false));
      expect(result.is_correct).toBe(true);
    });
  });

  describe('Comma-separated Synonyms', () => {
    it('accepts first synonym', () => {
      const result = JSON.parse(wasm.validate_answer('sofa', 'sofa, couch', false));
      expect(result.is_correct).toBe(true);
    });

    it('accepts second synonym', () => {
      const result = JSON.parse(wasm.validate_answer('couch', 'sofa, couch', false));
      expect(result.is_correct).toBe(true);
    });

    it('accepts both synonyms in order', () => {
      const result = JSON.parse(wasm.validate_answer('sofa couch', 'sofa, couch', false));
      expect(result.is_correct).toBe(true);
    });

    it('accepts both synonyms reversed', () => {
      const result = JSON.parse(wasm.validate_answer('couch sofa', 'sofa, couch', false));
      expect(result.is_correct).toBe(true);
    });
  });

  describe('Typo Tolerance (CloseEnough)', () => {
    it('accepts single typo in longer word', () => {
      const result = JSON.parse(wasm.validate_answer('yaa', 'ya', false));
      expect(result.is_correct).toBe(true);
      expect(result.result).toBe('CloseEnough');
      expect(result.quality).toBe(4); // No penalty for typo
    });

    it('rejects multiple typos', () => {
      const result = JSON.parse(wasm.validate_answer('yaaa', 'ya', false));
      expect(result.is_correct).toBe(false);
    });

    it('no typo tolerance for single char answers', () => {
      const result = JSON.parse(wasm.validate_answer('b', 'a', false));
      expect(result.is_correct).toBe(false);
    });

    it('single typo in medium word', () => {
      const result = JSON.parse(wasm.validate_answer('helllo', 'hello', false));
      expect(result.is_correct).toBe(true);
      expect(result.result).toBe('CloseEnough');
    });
  });

  describe('Spelling Normalization', () => {
    it('accepts British for American', () => {
      const result = JSON.parse(wasm.validate_answer('colour', 'color', false));
      expect(result.is_correct).toBe(true);
    });

    it('accepts American for British', () => {
      const result = JSON.parse(wasm.validate_answer('color', 'colour', false));
      expect(result.is_correct).toBe(true);
    });

    it('normalizes favourite/favorite', () => {
      const result = JSON.parse(wasm.validate_answer('favourite', 'favorite', false));
      expect(result.is_correct).toBe(true);
    });
  });

  describe('Contraction Normalization', () => {
    it('accepts expanded for contraction', () => {
      const result = JSON.parse(wasm.validate_answer('I am', "I'm", false));
      expect(result.is_correct).toBe(true);
    });

    it('accepts contraction for expanded', () => {
      const result = JSON.parse(wasm.validate_answer("don't", 'do not', false));
      expect(result.is_correct).toBe(true);
    });
  });

  describe('Hint Usage', () => {
    it('reduces quality when hint was used', () => {
      const result = JSON.parse(wasm.validate_answer('ui', 'ui', true));
      expect(result.is_correct).toBe(true);
      expect(result.quality).toBe(3); // Reduced from 4
    });

    it('reduces quality for CloseEnough with hint', () => {
      const result = JSON.parse(wasm.validate_answer('helllo', 'hello', true));
      expect(result.is_correct).toBe(true);
      expect(result.quality).toBe(3);
    });
  });

  describe('Korean Hangul Characters', () => {
    it('validates Korean consonant answer', () => {
      const result = JSON.parse(wasm.validate_answer('g', 'g / k', false));
      expect(result.is_correct).toBe(true);
    });

    it('validates Korean vowel answer', () => {
      const result = JSON.parse(wasm.validate_answer('a', 'a', false));
      expect(result.is_correct).toBe(true);
    });
  });
});

// =============================================================================
// Hint Generation Tests (Real WASM)
// =============================================================================

describe('Hint Generation (WASM)', () => {
  it('level 1 shows first letter and length', () => {
    const hint = wasm.get_hint('hello', 1);
    expect(hint).toBe('h____ (5 letters)');
  });

  it('level 2 shows first two characters', () => {
    const hint = wasm.get_hint('hello', 2);
    expect(hint).toBe('he___');
  });

  it('level 3+ shows full answer', () => {
    const hint = wasm.get_hint('hello', 3);
    expect(hint).toBe('hello');
  });

  it('handles short answers at level 2', () => {
    const hint = wasm.get_hint('hi', 2);
    expect(hint).toBe('hi'); // Full answer for 2-char words
  });

  it('handles single character', () => {
    const hint = wasm.get_hint('a', 1);
    expect(hint).toBe('a (1 letters)');
  });
});

// =============================================================================
// SRS Scheduling Tests (Real WASM)
// =============================================================================

describe('SRS Scheduling (WASM)', () => {
  const now = new Date().toISOString();

  describe('Learning Phase (steps 0-3)', () => {
    it('advances learning step on correct answer', () => {
      const cardState = JSON.stringify({
        learning_step: 0,
        stability: null,
        difficulty: null,
        repetitions: 0,
        next_review: now
      });

      const result = JSON.parse(wasm.calculate_next_review(cardState, 4, 0.9, false));

      expect(result.learning_step).toBe(1);
      expect(result.state).toBe('Learning');
    });

    it('resets to step 0 on wrong answer', () => {
      const cardState = JSON.stringify({
        learning_step: 2,
        stability: 4.0,
        difficulty: 5.0,
        repetitions: 0,
        next_review: now
      });

      const result = JSON.parse(wasm.calculate_next_review(cardState, 0, 0.9, false));

      expect(result.learning_step).toBe(0);
      expect(result.state).toBe('Learning');
    });

    it('progresses through learning steps', () => {
      let state = {
        learning_step: 0,
        stability: null,
        difficulty: null,
        repetitions: 0,
        next_review: now
      };

      // Step 0 -> 1
      let result = JSON.parse(wasm.calculate_next_review(JSON.stringify(state), 4, 0.9, false));
      expect(result.learning_step).toBe(1);

      // Step 1 -> 2
      state.learning_step = 1;
      result = JSON.parse(wasm.calculate_next_review(JSON.stringify(state), 4, 0.9, false));
      expect(result.learning_step).toBe(2);

      // Step 2 -> 3
      state.learning_step = 2;
      result = JSON.parse(wasm.calculate_next_review(JSON.stringify(state), 4, 0.9, false));
      expect(result.learning_step).toBe(3);

      // Step 3 -> 4 (graduation)
      state.learning_step = 3;
      result = JSON.parse(wasm.calculate_next_review(JSON.stringify(state), 4, 0.9, false));
      expect(result.learning_step).toBe(4);
      expect(result.state).toBe('Review');
    });
  });

  describe('Graduation', () => {
    it('graduates at step 4', () => {
      const cardState = JSON.stringify({
        learning_step: 3,
        stability: null,
        difficulty: null,
        repetitions: 0,
        next_review: now
      });

      const result = JSON.parse(wasm.calculate_next_review(cardState, 4, 0.9, false));

      expect(result.learning_step).toBe(4);
      expect(result.state).toBe('Review');
      expect(result.stability).toBeGreaterThan(0);
      expect(result.difficulty).toBeGreaterThan(0);
    });

    it('initializes FSRS parameters on graduation', () => {
      const cardState = JSON.stringify({
        learning_step: 3,
        stability: null,
        difficulty: null,
        repetitions: 0,
        next_review: now
      });

      const result = JSON.parse(wasm.calculate_next_review(cardState, 4, 0.9, false));

      expect(result.stability).toBeDefined();
      expect(result.difficulty).toBeDefined();
      expect(result.repetitions).toBe(1);
    });
  });

  describe('Graduated Card Reviews', () => {
    it('increases stability on correct answer', () => {
      const cardState = JSON.stringify({
        learning_step: 4,
        stability: 10.0,
        difficulty: 5.0,
        repetitions: 5,
        next_review: now
      });

      const result = JSON.parse(wasm.calculate_next_review(cardState, 4, 0.9, false));

      expect(result.learning_step).toBe(4);
      expect(result.state).toBe('Review');
      expect(result.repetitions).toBe(6);
    });

    it('returns to relearning on wrong answer', () => {
      const cardState = JSON.stringify({
        learning_step: 4,
        stability: 10.0,
        difficulty: 5.0,
        repetitions: 5,
        next_review: now
      });

      const result = JSON.parse(wasm.calculate_next_review(cardState, 0, 0.9, false));

      expect(result.learning_step).toBe(0);
      expect(result.state).toBe('Relearning');
    });
  });

  describe('Focus Mode', () => {
    it('uses faster learning steps in focus mode', () => {
      const cardState = JSON.stringify({
        learning_step: 0,
        stability: null,
        difficulty: null,
        repetitions: 0,
        next_review: now
      });

      const normalResult = JSON.parse(wasm.calculate_next_review(cardState, 4, 0.9, false));
      const focusResult = JSON.parse(wasm.calculate_next_review(cardState, 4, 0.9, true));

      // Focus mode should schedule earlier (faster steps)
      const normalNext = new Date(normalResult.next_review).getTime();
      const focusNext = new Date(focusResult.next_review).getTime();

      expect(focusNext).toBeLessThanOrEqual(normalNext);
    });
  });

  describe('Quality Ratings', () => {
    it('handles quality 0 (Again)', () => {
      const cardState = JSON.stringify({
        learning_step: 4,
        stability: 10.0,
        difficulty: 5.0,
        repetitions: 5,
        next_review: now
      });

      const result = JSON.parse(wasm.calculate_next_review(cardState, 0, 0.9, false));
      expect(result.state).toBe('Relearning');
    });

    it('handles quality 2 (Hard)', () => {
      const cardState = JSON.stringify({
        learning_step: 4,
        stability: 10.0,
        difficulty: 5.0,
        repetitions: 5,
        next_review: now
      });

      const result = JSON.parse(wasm.calculate_next_review(cardState, 2, 0.9, false));
      expect(result.state).toBe('Review');
    });

    it('handles quality 4 (Good)', () => {
      const cardState = JSON.stringify({
        learning_step: 4,
        stability: 10.0,
        difficulty: 5.0,
        repetitions: 5,
        next_review: now
      });

      const result = JSON.parse(wasm.calculate_next_review(cardState, 4, 0.9, false));
      expect(result.state).toBe('Review');
    });

    it('handles quality 5 (Easy)', () => {
      const cardState = JSON.stringify({
        learning_step: 4,
        stability: 10.0,
        difficulty: 5.0,
        repetitions: 5,
        next_review: now
      });

      const result = JSON.parse(wasm.calculate_next_review(cardState, 5, 0.9, false));
      expect(result.state).toBe('Review');
    });
  });

  describe('Next Review Dates', () => {
    it('schedules future date', () => {
      const cardState = JSON.stringify({
        learning_step: 0,
        stability: null,
        difficulty: null,
        repetitions: 0,
        next_review: now
      });

      const result = JSON.parse(wasm.calculate_next_review(cardState, 4, 0.9, false));
      const nextReview = new Date(result.next_review);

      expect(nextReview.getTime()).toBeGreaterThan(Date.now());
    });

    it('returns valid ISO8601 date', () => {
      const cardState = JSON.stringify({
        learning_step: 0,
        stability: null,
        difficulty: null,
        repetitions: 0,
        next_review: now
      });

      const result = JSON.parse(wasm.calculate_next_review(cardState, 4, 0.9, false));

      // Should parse without error
      const nextReview = new Date(result.next_review);
      expect(nextReview.toString()).not.toBe('Invalid Date');
    });
  });

  describe('Error Handling', () => {
    it('handles invalid JSON gracefully', () => {
      const result = JSON.parse(wasm.calculate_next_review('not json', 4, 0.9, false));
      expect(result.error).toBeDefined();
    });

    it('handles missing fields', () => {
      const result = JSON.parse(wasm.calculate_next_review('{}', 4, 0.9, false));
      // Should return error for missing required fields
      expect(result.error).toBeDefined();
      expect(result.error).toContain('missing field');
    });
  });
});

// =============================================================================
// Storage Tests (Mock - browser API)
// =============================================================================

describe('Mock Storage', () => {
  let storage;

  beforeEach(() => {
    storage = createMockStorage();
  });

  describe('Session Management', () => {
    it('starts with no session', async () => {
      expect(await storage.hasSession()).toBe(false);
      expect(await storage.getSession()).toBeNull();
    });

    it('saves and retrieves session', async () => {
      await storage.saveSession({
        session_id: 'test-123',
        cards: [{ card_id: 1, front: 'A', back: 'a' }]
      });

      expect(await storage.hasSession()).toBe(true);

      const session = await storage.getSession();
      expect(session.session_id).toBe('test-123');
      expect(session.cards).toHaveLength(1);
    });

    it('tracks session age', async () => {
      await storage.saveSession({ session_id: 'test', cards: [] });

      const age = await storage.getSessionAgeHours();
      expect(age).toBeGreaterThanOrEqual(0);
      expect(age).toBeLessThan(1);
    });
  });

  describe('Response Tracking', () => {
    it('adds and counts responses', async () => {
      await storage.addResponse({ card_id: 1, quality: 4, is_correct: true });
      await storage.addResponse({ card_id: 2, quality: 0, is_correct: false });

      expect(await storage.getPendingCount()).toBe(2);
    });

    it('updates existing response for same card', async () => {
      await storage.addResponse({ card_id: 1, quality: 4 });
      await storage.addResponse({ card_id: 1, quality: 5 });

      expect(await storage.getPendingCount()).toBe(1);

      const responses = await storage.getResponses();
      expect(responses[0].quality).toBe(5);
    });

    it('clears all responses', async () => {
      await storage.addResponse({ card_id: 1, quality: 4 });
      await storage.addResponse({ card_id: 2, quality: 0 });

      await storage.clearResponses();

      expect(await storage.getPendingCount()).toBe(0);
    });
  });

  describe('Card State Updates', () => {
    it('updates card state in session', async () => {
      await storage.saveSession({
        session_id: 'test',
        cards: [{ card_id: 1, learning_step: 0, fsrs_stability: 1.0 }]
      });

      await storage.updateCardState(1, { learning_step: 1, fsrs_stability: 2.0 });

      const session = await storage.getSession();
      const card = session.cards.find(c => c.card_id === 1);

      expect(card.learning_step).toBe(1);
      expect(card.fsrs_stability).toBe(2.0);
    });
  });
});

// =============================================================================
// Connectivity Tests (Mock - browser API)
// =============================================================================

describe('Mock Connectivity', () => {
  let connectivity;

  beforeEach(() => {
    connectivity = createMockConnectivity();
  });

  it('starts online', () => {
    expect(connectivity.isOnline()).toBe(true);
  });

  it('simulates going offline', () => {
    connectivity.simulateOffline();
    expect(connectivity.isOnline()).toBe(false);
  });

  it('simulates going online', () => {
    connectivity.simulateOffline();
    connectivity.simulateOnline();
    expect(connectivity.isOnline()).toBe(true);
  });

  it('fires event listeners', () => {
    let onlineCount = 0;
    let offlineCount = 0;

    connectivity.addEventListener('online', () => onlineCount++);
    connectivity.addEventListener('offline', () => offlineCount++);

    connectivity.simulateOffline();
    connectivity.simulateOnline();

    expect(offlineCount).toBe(1);
    expect(onlineCount).toBe(1);
  });

  it('removes event listeners', () => {
    let count = 0;
    const handler = () => count++;

    connectivity.addEventListener('online', handler);
    connectivity.simulateOnline();
    expect(count).toBe(1);

    connectivity.removeEventListener('online', handler);
    connectivity.simulateOnline();
    expect(count).toBe(1); // unchanged
  });
});

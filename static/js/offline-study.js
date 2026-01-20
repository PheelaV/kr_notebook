/**
 * Offline study controller.
 *
 * Handles the complete offline study experience:
 * - Loading cards from IndexedDB
 * - Running the study loop with WASM validation
 * - Managing reinforcement queue for failed cards
 * - Storing responses for later sync
 */

'use strict';

console.log('[OfflineStudy] Script loading...');

const OfflineStudy = (function() {
  // State
  let wasm = null;
  let session = null;
  let cardQueue = [];
  let reinforcementQueue = [];
  let currentCard = null;
  let hintsUsed = 0;
  let isStudying = false;
  let totalReviewed = 0;
  let correctCount = 0;
  let cardsSinceReinforcement = 0; // Counter for interleaving reinforcement cards

  // State for override functionality
  let lastUserAnswer = '';
  let lastValidation = null;
  let lastPreState = null; // Card state before the review was processed

  // DOM elements (populated on init)
  let elements = {};

  /**
   * Initialize the offline study system.
   * @returns {Promise<boolean>} True if ready to study
   */
  async function init() {
    console.log('[OfflineStudy] Initializing...');

    // Check for required capabilities
    const { supported, missing } = await window.OfflineCapability.check();
    if (!supported) {
      console.error('[OfflineStudy] Missing capabilities:', missing);
      return false;
    }

    // Load WASM module
    try {
      wasm = await window.OfflineCapability.initWasm();
      console.log('[OfflineStudy] WASM module loaded');
    } catch (error) {
      console.error('[OfflineStudy] Failed to load WASM:', error);
      return false;
    }

    // Load session from IndexedDB
    session = await window.OfflineStorage.getSession();
    if (!session) {
      console.log('[OfflineStudy] No offline session available');
      return false;
    }

    // Check session age
    const ageHours = await window.OfflineStorage.getSessionAgeHours();
    if (ageHours > 48) {
      console.warn('[OfflineStudy] Session is stale:', ageHours, 'hours old');
      // Still allow use but will show warning in UI
    }

    // Initialize card queues
    initializeQueues();

    console.log('[OfflineStudy] Ready with', cardQueue.length, 'cards');
    return true;
  }

  /**
   * Initialize card queues from session.
   */
  function initializeQueues() {
    // Start with all cards, prioritized by due date
    cardQueue = session.cards.slice().sort(function(a, b) {
      return new Date(a.next_review) - new Date(b.next_review);
    });
    reinforcementQueue = [];
    totalReviewed = 0;
    correctCount = 0;
    cardsSinceReinforcement = 0;
  }

  /**
   * Start the offline study session.
   * @param {Object} domElements - DOM elements for the study UI
   */
  function start(domElements) {
    elements = domElements;
    isStudying = true;

    // Set up keyboard shortcuts
    document.removeEventListener('keydown', handleKeyDown);
    document.addEventListener('keydown', handleKeyDown);

    showNextCard();
  }

  /**
   * Handle keyboard shortcuts.
   * @param {KeyboardEvent} e
   */
  function handleKeyDown(e) {
    if (!isStudying) return;

    // Skip if focused on input or textarea
    if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA') {
      // But allow Enter to submit from input
      if (e.code === 'Enter') {
        const input = document.querySelector('.answer-input');
        if (input && input.value) {
          e.preventDefault();
          submitAnswer(input.value);
        }
      }
      return;
    }

    // Number keys 1-4 for multiple choice
    var keyMap = {
      'Digit1': 0, 'Digit2': 1, 'Digit3': 2, 'Digit4': 3,
      'Numpad1': 0, 'Numpad2': 1, 'Numpad3': 2, 'Numpad4': 3
    };

    if (keyMap[e.code] !== undefined) {
      e.preventDefault();
      var btn = document.querySelector('.choice-btn[data-choice="' + keyMap[e.code] + '"]');
      if (btn) {
        btn.click();
      }
      return;
    }

    // Enter to advance to next card (when result is showing)
    if (e.code === 'Enter') {
      e.preventDefault();
      var continueBtn = document.querySelector('.continue-btn');
      if (continueBtn) {
        continueBtn.click();
        return;
      }
    }

    // H for hint
    if (e.code === 'KeyH') {
      e.preventDefault();
      showHint();
      return;
    }
  }

  /**
   * Stop the study session.
   */
  function stop() {
    isStudying = false;
    currentCard = null;
    document.removeEventListener('keydown', handleKeyDown);
  }

  /**
   * Get the next card to study.
   * Interleaves reinforcement cards - shows one every 3 regular cards.
   * @returns {Object|null} Next card or null if done
   */
  function getNextCard() {
    // If main queue is empty, use reinforcement queue
    if (cardQueue.length === 0 && reinforcementQueue.length > 0) {
      cardsSinceReinforcement = 0;
      return reinforcementQueue.shift();
    }

    // If reinforcement queue has cards and we've done 3+ cards since last one, interleave
    if (reinforcementQueue.length > 0 && cardsSinceReinforcement >= 3) {
      cardsSinceReinforcement = 0;
      return reinforcementQueue.shift();
    }

    // Main queue
    if (cardQueue.length > 0) {
      cardsSinceReinforcement++;
      return cardQueue.shift();
    }

    // Both queues empty
    return null;
  }

  /**
   * Show the next card in the UI.
   */
  function showNextCard() {
    currentCard = getNextCard();
    hintsUsed = 0;

    if (!currentCard) {
      showSessionComplete();
      return;
    }

    renderCard(currentCard);
  }

  /**
   * Render a card in the study UI.
   * @param {Object} card - The card to display
   */
  function renderCard(card) {
    if (!elements.cardContainer) return;

    const remaining = cardQueue.length + reinforcementQueue.length;
    const isReinforcement = reinforcementQueue.length > 0;

    // Update progress
    if (elements.progressText) {
      elements.progressText.textContent = `${totalReviewed} reviewed, ${remaining} remaining`;
    }

    // Build card HTML
    const hasChoices = card.choices && card.choices.length > 0;

    // NEVER show description during question phase - it often contains the answer
    // (e.g., "Like 's' in 'sun'" when the answer is "s")
    // Description is shown after answer is revealed in showResult()

    let html = `
      <div class="offline-card" data-card-id="${card.card_id}">
        <div class="card-front text-2xl font-bold mb-4 text-center">${escapeHtml(card.front)}</div>

        <div class="answer-section mt-6">
          ${hasChoices ? renderChoices(card.choices) : renderTextInput()}
        </div>

        <div class="hint-section mt-4 text-center">
          <button type="button" class="hint-btn text-sm text-indigo-600 dark:text-indigo-400 hover:underline" onclick="OfflineStudy.showHint()">
            Need a hint?
          </button>
          <div class="hint-display hidden mt-2 text-sm text-gray-600 dark:text-gray-400"></div>
        </div>

        <div class="result-section hidden mt-6"></div>
      </div>
    `;

    elements.cardContainer.innerHTML = html;

    // Focus input if text mode
    if (!hasChoices) {
      const input = elements.cardContainer.querySelector('.answer-input');
      if (input) {
        input.focus();
        input.addEventListener('keypress', function(e) {
          if (e.key === 'Enter') {
            submitAnswer(input.value);
          }
        });
      }
    }

    // Show reinforcement indicator
    if (elements.reinforcementBadge) {
      elements.reinforcementBadge.classList.toggle('hidden', !isReinforcement || reinforcementQueue.length === 0);
      if (isReinforcement && reinforcementQueue.length > 0) {
        elements.reinforcementBadge.textContent = `${reinforcementQueue.length + 1} to reinforce`;
      }
    }
  }

  /**
   * Render multiple choice options.
   * @param {Array} choices - Answer choices
   * @returns {string} HTML string
   */
  function renderChoices(choices) {
    return `
      <div class="choices-grid grid grid-cols-2 gap-3">
        ${choices.map(function(choice, i) {
          return `
            <button type="button"
                    class="choice-btn px-4 py-3 border-2 border-gray-300 dark:border-gray-600 rounded-lg
                           hover:border-indigo-500 dark:hover:border-indigo-400 transition-colors
                           text-lg font-medium"
                    onclick="OfflineStudy.submitAnswer('${escapeHtml(choice)}')"
                    data-choice="${i}">
              ${escapeHtml(choice)}
            </button>
          `;
        }).join('')}
      </div>
    `;
  }

  /**
   * Render text input for answers.
   * @returns {string} HTML string
   */
  function renderTextInput() {
    return `
      <div class="text-input-section">
        <input type="text"
               class="answer-input w-full px-4 py-3 text-lg border-2 border-gray-300 dark:border-gray-600
                      rounded-lg focus:border-indigo-500 dark:focus:border-indigo-400
                      dark:bg-gray-800 dark:text-white"
               placeholder="Type your answer..."
               autocomplete="off"
               autocapitalize="off">
        <button type="button"
                class="submit-btn mt-3 w-full px-4 py-3 bg-indigo-600 text-white rounded-lg
                       hover:bg-indigo-700 transition-colors font-medium"
                onclick="OfflineStudy.submitAnswer(document.querySelector('.answer-input').value)">
          Submit
        </button>
      </div>
    `;
  }

  /**
   * Show a hint for the current card.
   */
  function showHint() {
    if (!currentCard || !wasm) return;

    hintsUsed++;
    const hint = wasm.get_hint(currentCard.back, hintsUsed);

    const hintDisplay = elements.cardContainer.querySelector('.hint-display');
    if (hintDisplay) {
      hintDisplay.textContent = hint;
      hintDisplay.classList.remove('hidden');
    }
  }

  /**
   * Submit an answer for the current card.
   * @param {string} answer - User's answer
   */
  async function submitAnswer(answer) {
    if (!currentCard || !wasm || !isStudying) return;

    // Disable further input
    disableInput();

    // Save pre-state for potential override
    lastPreState = {
      learning_step: currentCard.learning_step,
      fsrs_stability: currentCard.fsrs_stability,
      fsrs_difficulty: currentCard.fsrs_difficulty,
      next_review: currentCard.next_review
    };
    lastUserAnswer = answer;

    // Validate answer using WASM
    const validationJson = wasm.validate_answer(answer, currentCard.back, hintsUsed > 0);
    const validation = JSON.parse(validationJson);
    lastValidation = validation;

    // Calculate new SRS state using WASM
    const cardState = JSON.stringify({
      learning_step: currentCard.learning_step,
      fsrs_stability: currentCard.fsrs_stability,
      fsrs_difficulty: currentCard.fsrs_difficulty,
      repetitions: currentCard.repetitions || 0,
      last_review: null
    });

    const newStateJson = wasm.calculate_next_review(
      cardState,
      validation.quality,
      session.desired_retention,
      session.focus_mode
    );
    const newState = JSON.parse(newStateJson);

    // Show result
    showResult(validation, newState);

    // Update stats
    totalReviewed++;
    if (validation.is_correct) {
      correctCount++;
    } else {
      // Add to reinforcement queue
      reinforcementQueue.push({
        ...currentCard,
        learning_step: newState.learning_step,
        fsrs_stability: newState.fsrs_stability,
        fsrs_difficulty: newState.fsrs_difficulty,
        next_review: newState.next_review
      });
    }

    // Store response in IndexedDB (with user answer for potential override)
    await window.OfflineStorage.addResponse({
      session_id: session.session_id,
      card_id: currentCard.card_id,
      quality: validation.quality,
      is_correct: validation.is_correct,
      hints_used: hintsUsed,
      timestamp: new Date().toISOString(),
      learning_step: newState.learning_step,
      fsrs_stability: newState.fsrs_stability,
      fsrs_difficulty: newState.fsrs_difficulty,
      next_review: newState.next_review,
      // Store for potential override
      user_answer: answer,
      original_result: validation.result,
      // Store pre-state for override restoration
      pre_learning_step: lastPreState.learning_step,
      pre_fsrs_stability: lastPreState.fsrs_stability,
      pre_fsrs_difficulty: lastPreState.fsrs_difficulty,
      pre_next_review: lastPreState.next_review
    });

    // Update card state in session (for consistency if card comes back in reinforcement)
    await window.OfflineStorage.updateCardState(currentCard.card_id, newState);

    // Update pending count display
    updatePendingCount();
  }

  /**
   * Disable input controls.
   */
  function disableInput() {
    const inputs = elements.cardContainer.querySelectorAll('.choice-btn, .answer-input, .submit-btn, .hint-btn');
    inputs.forEach(function(el) {
      el.disabled = true;
      el.classList.add('pointer-events-none', 'opacity-50');
    });
  }

  // Inline SVG icons for offline use
  const ICONS = {
    checkCircle: '<svg class="w-10 h-10 mx-auto" fill="currentColor" viewBox="0 0 24 24"><path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"/></svg>',
    closeCircle: '<svg class="w-10 h-10 mx-auto" fill="currentColor" viewBox="0 0 24 24"><path d="M12 2C6.47 2 2 6.47 2 12s4.47 10 10 10 10-4.47 10-10S17.53 2 12 2zm5 13.59L15.59 17 12 13.41 8.41 17 7 15.59 10.59 12 7 8.41 8.41 7 12 10.59 15.59 7 17 8.41 13.41 12 17 15.59z"/></svg>',
    checkDecagram: '<svg class="w-16 h-16 mx-auto" fill="currentColor" viewBox="0 0 24 24"><path d="M23,12L20.56,9.22L20.9,5.54L17.29,4.72L15.4,1.54L12,3L8.6,1.54L6.71,4.72L3.1,5.53L3.44,9.21L1,12L3.44,14.78L3.1,18.47L6.71,19.29L8.6,22.47L12,21L15.4,22.46L17.29,19.28L20.9,18.46L20.56,14.78L23,12M10,17L6,13L7.41,11.59L10,14.17L16.59,7.58L18,9L10,17Z"/></svg>',
    cloudUpload: '<svg class="w-8 h-8 mx-auto" fill="currentColor" viewBox="0 0 24 24"><path d="M19.35 10.04C18.67 6.59 15.64 4 12 4 9.11 4 6.6 5.64 5.35 8.04 2.34 8.36 0 10.91 0 14c0 3.31 2.69 6 6 6h13c2.76 0 5-2.24 5-5 0-2.64-2.05-4.78-4.65-4.96zM14 13v4h-4v-4H7l5-5 5 5h-3z"/></svg>'
  };

  /**
   * Show the result of a review.
   * @param {Object} validation - Validation result
   * @param {Object} newState - New SRS state
   */
  function showResult(validation, newState) {
    const resultSection = elements.cardContainer.querySelector('.result-section');
    if (!resultSection) return;

    const isCorrect = validation.is_correct;
    const resultClass = isCorrect ? 'text-green-600 dark:text-green-400' : 'text-red-600 dark:text-red-400';
    const resultIcon = isCorrect ? ICONS.checkCircle : ICONS.closeCircle;
    const resultText = validation.result === 'CloseEnough' ? 'Close enough!' : (isCorrect ? 'Correct!' : 'Incorrect');

    // Show description after answer reveal (hidden during question to avoid giving away answer)
    const descriptionHtml = currentCard.description
      ? `<div class="text-sm text-gray-500 dark:text-gray-400 mt-1">${escapeHtml(currentCard.description)}</div>`
      : '';

    resultSection.innerHTML = `
      <div class="result-display ${resultClass} text-center">
        ${resultIcon}
        <div class="text-xl font-bold mt-2">${resultText}</div>
        ${!isCorrect ? `<div class="correct-answer mt-2">Correct answer: <strong>${escapeHtml(currentCard.back)}</strong></div>` : ''}
        ${descriptionHtml}
      </div>

      <!-- Override section (hidden initially) -->
      <div id="override-section" class="hidden mt-4 pt-4 border-t border-gray-200 dark:border-gray-700">
        <div class="mb-3">
          <label class="block text-xs text-gray-500 dark:text-gray-400 mb-1">
            Your answer (edit if needed):
          </label>
          <input type="text" id="suggested-answer"
                 class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded
                        bg-white dark:bg-gray-700 text-gray-900 dark:text-white
                        focus:outline-none focus:ring-1 focus:ring-indigo-500"
                 value="${escapeHtml(lastUserAnswer)}"
                 placeholder="What should be accepted?">
        </div>

        <p class="text-sm text-gray-600 dark:text-gray-400 mb-2">How would you rate your answer?</p>
        <div class="flex flex-wrap justify-center gap-2">
          <button type="button" onclick="OfflineStudy.submitOverride(0)"
                  class="px-3 py-1.5 text-sm bg-red-100 hover:bg-red-200 dark:bg-red-900/30 dark:hover:bg-red-900/50 text-red-800 dark:text-red-300 rounded">
            Wrong
          </button>
          <button type="button" onclick="OfflineStudy.submitOverride(2)"
                  class="px-3 py-1.5 text-sm bg-yellow-100 hover:bg-yellow-200 dark:bg-yellow-900/30 dark:hover:bg-yellow-900/50 text-yellow-800 dark:text-yellow-300 rounded">
            Hard
          </button>
          <button type="button" onclick="OfflineStudy.submitOverride(4)"
                  class="px-3 py-1.5 text-sm bg-green-100 hover:bg-green-200 dark:bg-green-900/30 dark:hover:bg-green-900/50 text-green-800 dark:text-green-300 rounded">
            Correct
          </button>
          <button type="button" onclick="OfflineStudy.submitOverride(5)"
                  class="px-3 py-1.5 text-sm bg-blue-100 hover:bg-blue-200 dark:bg-blue-900/30 dark:hover:bg-blue-900/50 text-blue-800 dark:text-blue-300 rounded">
            Easy
          </button>
        </div>
      </div>

      <!-- Toggle button -->
      <button type="button" id="show-override-btn" onclick="OfflineStudy.showOverrideSection()"
              class="mt-3 text-xs text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300 underline block mx-auto">
        Override ruling
      </button>

      <button type="button"
              class="continue-btn mt-4 w-full px-4 py-3 bg-indigo-600 text-white rounded-lg
                     hover:bg-indigo-700 transition-colors font-medium"
              onclick="OfflineStudy.showNextCard()">
        ${cardQueue.length + reinforcementQueue.length > 0 ? 'Next Card' : 'Finish'}
      </button>
    `;

    resultSection.classList.remove('hidden');

    // Highlight correct/incorrect choice
    if (currentCard.choices && currentCard.choices.length > 0) {
      const choiceBtns = elements.cardContainer.querySelectorAll('.choice-btn');
      choiceBtns.forEach(function(btn) {
        const choiceText = btn.textContent.trim();
        if (choiceText === currentCard.back) {
          btn.classList.add('border-green-500', 'bg-green-50', 'dark:bg-green-900/20');
        } else if (!isCorrect && btn.getAttribute('onclick').includes(escapeHtml(choiceText))) {
          // User's wrong choice
          btn.classList.add('border-red-500', 'bg-red-50', 'dark:bg-red-900/20');
        }
      });
    }
  }

  /**
   * Show the override section.
   */
  function showOverrideSection() {
    const overrideSection = document.getElementById('override-section');
    const showBtn = document.getElementById('show-override-btn');
    if (overrideSection) {
      overrideSection.classList.remove('hidden');
    }
    if (showBtn) {
      showBtn.classList.add('hidden');
    }
  }

  /**
   * Submit an override for the current card.
   * @param {number} quality - Override quality (0=Wrong, 2=Hard, 4=Correct, 5=Easy)
   */
  async function submitOverride(quality) {
    if (!currentCard || !lastPreState) return;

    const suggestedAnswer = document.getElementById('suggested-answer')?.value || lastUserAnswer;
    const isCorrectOverride = quality >= 4; // Correct or Easy

    // Calculate new state based on override quality
    const cardState = JSON.stringify({
      learning_step: lastPreState.learning_step,
      fsrs_stability: lastPreState.fsrs_stability,
      fsrs_difficulty: lastPreState.fsrs_difficulty,
      repetitions: currentCard.repetitions || 0,
      last_review: null
    });

    const newStateJson = wasm.calculate_next_review(
      cardState,
      quality,
      session.desired_retention,
      session.focus_mode
    );
    const newState = JSON.parse(newStateJson);

    // Update stats if override changes correctness
    if (isCorrectOverride && !lastValidation.is_correct) {
      // Was marked wrong, now marked correct - fix stats
      correctCount++;
      // Remove from reinforcement queue if present
      const idx = reinforcementQueue.findIndex(c => c.card_id === currentCard.card_id);
      if (idx !== -1) {
        reinforcementQueue.splice(idx, 1);
      }
    } else if (!isCorrectOverride && lastValidation.is_correct) {
      // Was marked correct, now marked wrong - fix stats
      correctCount--;
      // Add to reinforcement queue
      reinforcementQueue.push({
        ...currentCard,
        learning_step: newState.learning_step,
        fsrs_stability: newState.fsrs_stability,
        fsrs_difficulty: newState.fsrs_difficulty,
        next_review: newState.next_review
      });
    }

    // Store override response in IndexedDB
    await window.OfflineStorage.addResponse({
      session_id: session.session_id,
      card_id: currentCard.card_id,
      quality: quality,
      is_correct: isCorrectOverride,
      hints_used: hintsUsed,
      timestamp: new Date().toISOString(),
      learning_step: newState.learning_step,
      fsrs_stability: newState.fsrs_stability,
      fsrs_difficulty: newState.fsrs_difficulty,
      next_review: newState.next_review,
      // Override-specific fields
      is_override: true,
      user_answer: lastUserAnswer,
      original_result: lastValidation.result,
      suggested_answer: suggestedAnswer,
      // Pre-state for server-side restoration
      pre_learning_step: lastPreState.learning_step,
      pre_fsrs_stability: lastPreState.fsrs_stability,
      pre_fsrs_difficulty: lastPreState.fsrs_difficulty,
      pre_next_review: lastPreState.next_review
    });

    // Update card state in session
    await window.OfflineStorage.updateCardState(currentCard.card_id, newState);

    // Update pending count display
    updatePendingCount();

    // Update UI to reflect override
    const resultSection = elements.cardContainer.querySelector('.result-section');
    if (resultSection) {
      const overrideSection = document.getElementById('override-section');
      if (overrideSection) {
        overrideSection.innerHTML = `
          <div class="text-center text-sm text-indigo-600 dark:text-indigo-400">
            <span>Override saved - will sync when online</span>
          </div>
        `;
      }
    }
  }

  /**
   * Show session complete screen.
   */
  function showSessionComplete() {
    isStudying = false;
    document.removeEventListener('keydown', handleKeyDown);

    if (!elements.cardContainer) return;

    const accuracy = totalReviewed > 0 ? Math.round((correctCount / totalReviewed) * 100) : 0;

    elements.cardContainer.innerHTML = `
      <div class="session-complete text-center py-8">
        <div class="text-green-500">${ICONS.checkDecagram}</div>
        <h2 class="text-2xl font-bold mt-4">Session Complete!</h2>
        <div class="stats mt-4 text-gray-600 dark:text-gray-400">
          <div>Cards reviewed: <strong>${totalReviewed}</strong></div>
          <div>Accuracy: <strong>${accuracy}%</strong></div>
        </div>
        <div class="sync-info mt-6 p-4 bg-yellow-50 dark:bg-yellow-900/20 rounded-lg">
          <div class="text-yellow-600">${ICONS.cloudUpload}</div>
          <p class="mt-2 text-sm text-yellow-800 dark:text-yellow-200">
            Your progress is saved locally. It will sync automatically when you're back online.
          </p>
        </div>
        <button type="button"
                class="mt-6 px-6 py-3 bg-indigo-600 text-white rounded-lg hover:bg-indigo-700 transition-colors"
                onclick="window.location.href = '/'">
          Return Home
        </button>
      </div>
    `;
  }

  /**
   * Update the pending sync count display.
   */
  async function updatePendingCount() {
    const count = await window.OfflineStorage.getPendingCount();
    if (elements.pendingCount) {
      elements.pendingCount.textContent = count;
    }
    // Dispatch event for other components
    window.dispatchEvent(new CustomEvent('offline-pending-update', { detail: { count } }));
  }

  /**
   * Escape HTML to prevent XSS.
   * @param {string} str - String to escape
   * @returns {string} Escaped string
   */
  function escapeHtml(str) {
    if (!str) return '';
    return str
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#039;');
  }

  /**
   * Get current session info.
   * @returns {Object|null} Session info or null
   */
  function getSessionInfo() {
    if (!session) return null;
    return {
      session_id: session.session_id,
      created_at: session.created_at,
      total_cards: session.cards ? session.cards.length : 0,
      remaining: cardQueue.length + reinforcementQueue.length,
      reviewed: totalReviewed,
      correct: correctCount
    };
  }

  /**
   * Check if offline study is available.
   * @returns {Promise<boolean>}
   */
  async function isAvailable() {
    try {
      const { supported } = await window.OfflineCapability.check();
      if (!supported) return false;
      return await window.OfflineStorage.hasSession();
    } catch (e) {
      return false;
    }
  }

  // Public API
  return {
    init: init,
    start: start,
    stop: stop,
    showNextCard: showNextCard,
    showHint: showHint,
    submitAnswer: submitAnswer,
    showOverrideSection: showOverrideSection,
    submitOverride: submitOverride,
    getSessionInfo: getSessionInfo,
    isAvailable: isAvailable,
    updatePendingCount: updatePendingCount
  };
})();

console.log('[OfflineStudy] Module created:', typeof OfflineStudy);

// Export for use in other modules
window.OfflineStudy = OfflineStudy;
console.log('[OfflineStudy] Exported to window');

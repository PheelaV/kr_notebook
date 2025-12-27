/**
 * Card Interactions Module
 *
 * Handles multiple choice selection, keyboard navigation, and form interactions
 * for study and practice card templates.
 *
 * Uses event delegation from document.body so handlers survive HTMX swaps.
 */
(function() {
  'use strict';

  // State - reset on each card load
  var selectedAnswer = null;

  // CSS classes for selection state
  var SELECTED_CLASSES = ['border-indigo-500', 'bg-indigo-100', 'dark:bg-indigo-900', 'selected'];
  var UNSELECTED_CLASSES = ['border-transparent'];

  // Hint system state
  var currentHint = 0;
  var hintsUsed = 0;
  var hints = [];

  /**
   * Select an answer choice
   */
  function selectAnswer(btn, answer) {
    // Clear previous selection
    document.querySelectorAll('.choice-btn').forEach(function(b) {
      SELECTED_CLASSES.forEach(function(cls) { b.classList.remove(cls); });
      UNSELECTED_CLASSES.forEach(function(cls) { b.classList.add(cls); });
    });

    // Mark this button as selected
    UNSELECTED_CLASSES.forEach(function(cls) { btn.classList.remove(cls); });
    SELECTED_CLASSES.forEach(function(cls) { btn.classList.add(cls); });

    // Set hidden input value
    var answerInput = document.getElementById('answer-input');
    if (answerInput) {
      answerInput.value = answer;
    }
    selectedAnswer = answer;

    // Enable submit button
    var submitBtn = document.getElementById('submit-btn');
    if (submitBtn) {
      submitBtn.disabled = false;
    }
  }

  /**
   * Validate multiple choice form before submission
   */
  function validateMultipleChoice() {
    var answerInput = document.getElementById('answer-input');
    if (!selectedAnswer || !answerInput || !answerInput.value) {
      return false;
    }
    return true;
  }

  /**
   * Initialize hint data from template
   */
  function initHints(hint1, hint2, hintFinal, initialHintsUsed) {
    hints = [hint1, hint2, hintFinal].filter(function(h) { return h; });
    currentHint = 0;
    hintsUsed = initialHintsUsed || 0;
  }

  /**
   * Show next hint
   */
  function showHint() {
    var hintArea = document.getElementById('hint-area');
    var hintText = document.getElementById('hint-text');
    var hintsUsedInput = document.getElementById('hints-used');

    if (currentHint < hints.length && hintArea && hintText) {
      hintText.textContent = hints[currentHint];
      hintArea.classList.remove('hidden');
      currentHint++;
      hintsUsed = currentHint;
      if (hintsUsedInput) {
        hintsUsedInput.value = hintsUsed;
      }
    }
  }

  /**
   * Reset state for new card
   */
  function resetState() {
    selectedAnswer = null;
    currentHint = 0;
    hintsUsed = 0;
    hints = [];
  }

  // Expose to global scope IMMEDIATELY so templates can use these functions
  window.CardInteractions = {
    selectAnswer: selectAnswer,
    validateMultipleChoice: validateMultipleChoice,
    showHint: showHint,
    initHints: initHints,
    resetState: resetState
  };

  // Set up event listeners when DOM is ready
  function setupEventListeners() {
    // Event delegation: Click on choice buttons
    document.body.addEventListener('click', function(e) {
      var btn = e.target.closest('.choice-btn');
      if (btn && btn.dataset.choice) {
        e.preventDefault();
        selectAnswer(btn, btn.dataset.choice);
      }
    });

    // Event delegation: Keyboard navigation
    document.body.addEventListener('keydown', function(e) {
      // Skip if focused on input or textarea
      if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA') {
        return;
      }

      // Number keys 1-4 for multiple choice
      var keyMap = {
        'Digit1': 1, 'Digit2': 2, 'Digit3': 3, 'Digit4': 4,
        'Numpad1': 1, 'Numpad2': 2, 'Numpad3': 3, 'Numpad4': 4
      };

      if (keyMap[e.code]) {
        e.preventDefault();
        var btn = document.getElementById('choice-' + keyMap[e.code]);
        if (btn && btn.dataset.choice) {
          selectAnswer(btn, btn.dataset.choice);
        }
        return;
      }

      // Enter to submit (when answer selected or for next card)
      if (e.code === 'Enter') {
        e.preventDefault();

        // Check for result phase forms FIRST (more specific, takes priority)
        var nextForm = document.getElementById('next-card-form') || document.getElementById('practice-next-form');
        if (nextForm) {
          if (typeof htmx !== 'undefined') {
            htmx.trigger(nextForm, 'submit');
          } else {
            nextForm.requestSubmit();
          }
          return;
        }

        // Check for answer form (input phase) - only if no result form found
        var answerForm = document.getElementById('answer-form');
        if (answerForm && selectedAnswer) {
          answerForm.requestSubmit();
          return;
        }
      }

      // H for hint (when hint system is present)
      if (e.code === 'KeyH') {
        var hintArea = document.getElementById('hint-area');
        if (hintArea) {
          e.preventDefault();
          showHint();
        }
      }
    });

    // Reset state after HTMX swaps
    document.body.addEventListener('htmx:afterSwap', function(e) {
      // Reset state if swapping involves card container
      var cardContainer = document.getElementById('card-container');
      if (cardContainer || e.detail.target.id === 'card-container') {
        resetState();

        // Auto-focus text input if present
        setTimeout(function() {
          var textInput = document.getElementById('answer-input');
          if (textInput && textInput.type === 'text') {
            textInput.focus();
          }
        }, 0);
      }
    });
  }

  // Initialize when DOM is ready
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', setupEventListeners);
  } else {
    setupEventListeners();
  }

})();

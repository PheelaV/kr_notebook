/**
 * Vocabulary Search Module
 *
 * Client-side fuzzy search for vocabulary library using Fuse.js.
 * Shows clickable search results ordered by relevance.
 *
 * Weighted field priorities:
 * - romanization: 0.40 (primary for typing searches)
 * - term: 0.25 (Korean text, for copy-paste)
 * - translation: 0.20 (English meaning)
 * - notes: 0.05
 * - usages_text: 0.05
 * - examples_text: 0.05
 */
(function() {
  'use strict';

  // Fuse.js configuration
  var FUSE_OPTIONS = {
    keys: [
      { name: 'romanization', weight: 0.40 },
      { name: 'term', weight: 0.25 },
      { name: 'translation', weight: 0.20 },
      { name: 'notes', weight: 0.05 },
      { name: 'usages_text', weight: 0.05 },
      { name: 'examples_text', weight: 0.05 }
    ],
    threshold: 0.4,
    ignoreLocation: true,
    minMatchCharLength: 2,
    includeScore: true
  };

  var MAX_RESULTS = 20;

  var fuse = null;
  var searchInput = null;
  var clearButton = null;
  var resultCount = null;
  var resultsContainer = null;
  var backToSearchBtn = null;
  var searchContainer = null;
  var totalCount = 0;
  var debounceTimer = null;
  var initRetries = 0;
  var MAX_INIT_RETRIES = 50; // 5 seconds max wait for Fuse.js

  /**
   * Initialize the search functionality
   */
  function init() {
    // Check Fuse.js is loaded (from CDN)
    if (typeof Fuse === 'undefined') {
      if (initRetries < MAX_INIT_RETRIES) {
        initRetries++;
        setTimeout(init, 100);
      }
      return;
    }

    if (typeof window.VocabularyData === 'undefined' || !window.VocabularyData.length) {
      return;
    }

    searchInput = document.getElementById('vocab-search-input');
    clearButton = document.getElementById('vocab-search-clear');
    resultCount = document.getElementById('vocab-result-count');
    resultsContainer = document.getElementById('vocab-search-results');
    backToSearchBtn = document.getElementById('vocab-back-to-search');
    searchContainer = document.getElementById('vocab-search-container');

    if (!searchInput || !resultsContainer) {
      return;
    }

    // Back to search button handler
    if (backToSearchBtn) {
      backToSearchBtn.addEventListener('click', backToSearch);
    }

    totalCount = window.VocabularyData.length;

    // Initialize Fuse.js
    fuse = new Fuse(window.VocabularyData, FUSE_OPTIONS);

    // Bind event handlers
    searchInput.addEventListener('input', handleSearchInput);
    searchInput.addEventListener('keydown', handleKeydown);
    searchInput.addEventListener('focus', handleFocus);

    if (clearButton) {
      clearButton.addEventListener('click', clearSearch);
    }

    // Close results when clicking outside
    document.addEventListener('click', handleClickOutside);

    // Global keyboard shortcut: "/" to focus search
    document.addEventListener('keydown', handleGlobalKeydown);

    // Update initial count display
    updateResultCount(totalCount, totalCount);

    // Mark search as ready (for E2E tests)
    searchInput.dataset.searchReady = 'true';
  }

  /**
   * Handle search input with debounce
   */
  function handleSearchInput() {
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(performSearch, 150);

    // Show/hide clear button
    if (clearButton) {
      if (searchInput.value.length > 0) {
        clearButton.classList.remove('hidden');
      } else {
        clearButton.classList.add('hidden');
      }
    }
  }

  /**
   * Handle keydown in search input
   */
  function handleKeydown(e) {
    if (e.key === 'Escape') {
      clearSearch();
      searchInput.blur();
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      focusFirstResult();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      // Select the first result
      var first = resultsContainer.querySelector('.vocab-result');
      if (first) {
        first.click();
      }
    }
  }

  /**
   * Handle focus on search input
   */
  function handleFocus() {
    if (searchInput.value.trim().length >= 2) {
      performSearch();
    }
  }

  /**
   * Handle clicks outside the search area
   */
  function handleClickOutside(e) {
    if (!searchInput.contains(e.target) && !resultsContainer.contains(e.target)) {
      hideResults();
    }
    // Hide back to search button when clicking anywhere except:
    // - the button itself
    // - search results (which trigger navigation and show the button)
    // - the search container
    if (backToSearchBtn &&
        !backToSearchBtn.contains(e.target) &&
        !resultsContainer.contains(e.target) &&
        !searchContainer.contains(e.target)) {
      hideBackToSearchButton();
    }
  }

  /**
   * Handle global keyboard shortcuts
   */
  function handleGlobalKeydown(e) {
    if (e.key === '/' && !isInputFocused()) {
      e.preventDefault();
      // Focus immediately, scroll only if needed
      searchInput.focus();
      // Scroll into view if search is off-screen
      if (searchContainer && !isElementInViewport(searchContainer)) {
        searchContainer.scrollIntoView({ behavior: 'smooth', block: 'start' });
      }
      hideBackToSearchButton();
    }
  }

  /**
   * Check if an element is visible in the viewport
   */
  function isElementInViewport(el) {
    var rect = el.getBoundingClientRect();
    return rect.top >= 0 && rect.top < window.innerHeight;
  }

  /**
   * Check if an input element is currently focused
   */
  function isInputFocused() {
    var active = document.activeElement;
    return active && (
      active.tagName === 'INPUT' ||
      active.tagName === 'TEXTAREA' ||
      active.isContentEditable
    );
  }

  /**
   * Perform the search and show results
   */
  function performSearch() {
    var query = searchInput.value.trim();

    if (query.length < 2) {
      hideResults();
      updateResultCount(totalCount, totalCount);
      return;
    }

    var results = fuse.search(query).slice(0, MAX_RESULTS);
    var totalMatches = fuse.search(query).length;

    if (results.length === 0) {
      showNoResults();
      updateResultCount(0, totalCount);
    } else {
      renderResults(results, totalMatches);
      updateResultCount(totalMatches, totalCount);
    }
  }

  /**
   * Render search results
   */
  function renderResults(results, totalMatches) {
    var html = '<ul class="divide-y divide-gray-100 dark:divide-gray-700">';

    results.forEach(function(result, index) {
      var item = result.item;
      // SRS status indicator dot (populated by lazy fetch)
      var srsIndicator = '';
      if (item.srs_status === 'learning') {
        srsIndicator = '<span class="inline-block w-2.5 h-2.5 rounded-full bg-yellow-400 shrink-0" title="Learning"></span>';
      } else if (item.srs_status === 'graduated') {
        srsIndicator = '<span class="inline-block w-2.5 h-2.5 rounded-full bg-green-400 shrink-0" title="Graduated"></span>';
      }
      html += '<li>' +
        '<button type="button" ' +
        'class="vocab-result w-full text-left px-4 py-3 hover:bg-indigo-50 dark:hover:bg-indigo-900/30 focus:bg-indigo-50 dark:focus:bg-indigo-900/30 focus:outline-none focus:ring-2 focus:ring-inset focus:ring-indigo-500 transition-colors" ' +
        'data-vocab-id="' + escapeHtml(item.id) + '" ' +
        'data-index="' + index + '" ' +
        'onclick="window.VocabSearch.navigateToEntry(\'' + escapeHtml(item.id) + '\')">' +
        '<div class="flex items-center gap-3">' +
        srsIndicator +
        '<span class="text-xl font-bold text-gray-900 dark:text-white">' + escapeHtml(item.term) + '</span>' +
        '<span class="text-gray-500 dark:text-gray-400 text-sm">(' + escapeHtml(item.romanization) + ')</span>' +
        '</div>' +
        '<div class="text-indigo-600 dark:text-indigo-400 text-sm mt-0.5">' + escapeHtml(item.translation) + '</div>' +
        (item.notes ? '<div class="text-gray-500 dark:text-gray-500 text-xs mt-1 truncate">' + escapeHtml(item.notes) + '</div>' : '') +
        '</button>' +
        '</li>';
    });

    html += '</ul>';

    if (totalMatches > MAX_RESULTS) {
      html += '<div class="px-4 py-2 text-xs text-gray-500 dark:text-gray-400 bg-gray-50 dark:bg-gray-800 border-t border-gray-100 dark:border-gray-700">' +
        'Showing ' + MAX_RESULTS + ' of ' + totalMatches + ' matches. Type more to narrow results.' +
        '</div>';
    }

    resultsContainer.innerHTML = html;
    resultsContainer.classList.remove('hidden');
    hideBackToSearchButton();

    // Add keyboard navigation to results
    resultsContainer.querySelectorAll('.vocab-result').forEach(function(btn, idx) {
      btn.addEventListener('keydown', function(e) {
        handleResultKeydown(e, idx);
      });
    });
  }

  /**
   * Handle keyboard navigation within results
   */
  function handleResultKeydown(e, currentIndex) {
    var results = resultsContainer.querySelectorAll('.vocab-result');

    if (e.key === 'ArrowDown') {
      e.preventDefault();
      var next = results[currentIndex + 1];
      if (next) next.focus();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (currentIndex === 0) {
        searchInput.focus();
      } else {
        results[currentIndex - 1].focus();
      }
    } else if (e.key === 'Escape') {
      searchInput.focus();
      hideResults();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      results[currentIndex].click();
    }
  }

  /**
   * Focus the first result
   */
  function focusFirstResult() {
    var first = resultsContainer.querySelector('.vocab-result');
    if (first) first.focus();
  }

  /**
   * Navigate to a vocabulary entry
   */
  function navigateToEntry(vocabId) {
    // Select the <details> element, not the button in the dropdown
    var entry = document.querySelector('details[data-vocab-id="' + vocabId + '"]');

    if (!entry) {
      return;
    }

    // Find and open parent lesson details
    var lessonDetails = entry.closest('[data-lesson-section]');
    if (lessonDetails && !lessonDetails.open) {
      lessonDetails.open = true;
    }

    // Open the entry itself
    if (!entry.open) {
      entry.open = true;
    }

    // Scroll into view
    entry.scrollIntoView({ behavior: 'smooth', block: 'center' });

    // Flash highlight (CSS animation handles the effect)
    entry.classList.add('vocab-highlight');
    setTimeout(function() {
      entry.classList.remove('vocab-highlight');
    }, 2000);

    // Hide results and show back button
    hideResults();
    showBackToSearchButton();
  }

  /**
   * Show the "Back to Search" floating button
   */
  function showBackToSearchButton() {
    if (backToSearchBtn) {
      backToSearchBtn.classList.remove('hidden');
    }
  }

  /**
   * Hide the "Back to Search" floating button
   */
  function hideBackToSearchButton() {
    if (backToSearchBtn) {
      backToSearchBtn.classList.add('hidden');
    }
  }

  /**
   * Navigate back to search: scroll to search, focus input
   */
  function backToSearch() {
    if (searchContainer) {
      searchContainer.scrollIntoView({ behavior: 'smooth', block: 'start' });
    }
    // Focus immediately - scrollIntoView handles scroll asynchronously
    searchInput.focus();
    hideBackToSearchButton();
  }

  /**
   * Show "no results" in the dropdown
   */
  function showNoResults() {
    resultsContainer.innerHTML =
      '<div class="px-4 py-8 text-center">' +
      '<p class="text-gray-600 dark:text-gray-400">No vocabulary matches your search.</p>' +
      '<p class="text-sm text-gray-500 dark:text-gray-500 mt-1">Try a different spelling or search term.</p>' +
      '</div>';
    resultsContainer.classList.remove('hidden');
    hideBackToSearchButton();
  }

  /**
   * Hide results dropdown
   */
  function hideResults() {
    resultsContainer.classList.add('hidden');
  }

  /**
   * Clear search and hide results
   */
  function clearSearch() {
    searchInput.value = '';
    if (clearButton) {
      clearButton.classList.add('hidden');
    }
    hideResults();
    updateResultCount(totalCount, totalCount);
  }

  /**
   * Update the result count display
   */
  function updateResultCount(count, total) {
    if (resultCount) {
      if (count === total) {
        resultCount.textContent = total + ' words';
      } else {
        resultCount.textContent = count + ' of ' + total + ' matches';
      }
    }
  }

  /**
   * Escape HTML to prevent XSS
   */
  function escapeHtml(str) {
    if (!str) return '';
    return str
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#39;');
  }

  // Expose navigateToEntry for onclick handlers
  window.VocabSearch = {
    navigateToEntry: navigateToEntry
  };

  // Initialize when DOM is ready
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();

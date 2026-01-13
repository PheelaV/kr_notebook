/**
 * Offline mode feature detection and capability checking.
 *
 * Checks browser support for:
 * - WebAssembly (for FSRS scheduling)
 * - IndexedDB (for local storage)
 * - Service Worker (for offline interception)
 */

const OfflineCapability = {
  /**
   * Check if all required features are supported.
   * @returns {Promise<{supported: boolean, missing: string[]}>}
   */
  async check() {
    const missing = [];

    // Check WebAssembly
    if (typeof WebAssembly !== 'object' || typeof WebAssembly.instantiate !== 'function') {
      missing.push('WebAssembly');
    }

    // Check IndexedDB
    if (!window.indexedDB) {
      missing.push('IndexedDB');
    }

    // Check Service Worker
    if (!('serviceWorker' in navigator)) {
      missing.push('Service Worker');
    }

    return {
      supported: missing.length === 0,
      missing
    };
  },

  /**
   * Get a human-readable capability status string.
   * @returns {Promise<string>}
   */
  async getStatusHtml() {
    const { supported, missing } = await this.check();

    if (supported) {
      return `
        <span class="text-green-600 dark:text-green-400">
          <iconify-icon icon="mdi:check-circle" class="align-middle"></iconify-icon>
          All features supported
        </span>
      `;
    }

    return `
      <span class="text-red-600 dark:text-red-400">
        <iconify-icon icon="mdi:alert-circle" class="align-middle"></iconify-icon>
        Missing: ${missing.join(', ')}
      </span>
    `;
  },

  /**
   * Initialize the WASM module.
   * @returns {Promise<object>} The initialized WASM module
   */
  async initWasm() {
    const { default: init, validate_answer, calculate_next_review, get_hint } =
      await import('/static/wasm/offline_srs.js');

    await init();

    return { validate_answer, calculate_next_review, get_hint };
  }
};

// Export for use in other modules
window.OfflineCapability = OfflineCapability;

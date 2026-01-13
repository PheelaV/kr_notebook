/* tslint:disable */
/* eslint-disable */

/**
 * Calculate the next review schedule for a card.
 *
 * card_state_json: JSON with {learning_step, stability, difficulty, repetitions, next_review}
 * quality: 0=Again, 2=Hard, 4=Good, 5=Easy
 * desired_retention: Target retention rate (e.g., 0.9 for 90%)
 * focus_mode: If true, uses faster learning steps
 *
 * Returns JSON with new scheduling state.
 */
export function calculate_next_review(card_state_json: string, quality: number, desired_retention: number, focus_mode: boolean): string;

/**
 * Get hint for an answer (progressive reveal)
 */
export function get_hint(answer: string, level: number): string;

/**
 * Initialize panic hook for better error messages in browser console
 */
export function init(): void;

/**
 * Validate a user's answer against the correct answer.
 *
 * Returns JSON: {"result": "Correct"|"CloseEnough"|"Incorrect", "quality": 0-4}
 */
export function validate_answer(user_input: string, correct_answer: string, used_hint: boolean): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly calculate_next_review: (a: number, b: number, c: number, d: number, e: number) => [number, number];
  readonly get_hint: (a: number, b: number, c: number) => [number, number];
  readonly init: () => void;
  readonly validate_answer: (a: number, b: number, c: number, d: number, e: number) => [number, number];
  readonly __wbindgen_free: (a: number, b: number, c: number) => void;
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_externrefs: WebAssembly.Table;
  readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
* Instantiates the given `module`, which can either be bytes or
* a precompiled `WebAssembly.Module`.
*
* @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
*
* @returns {InitOutput}
*/
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
* If `module_or_path` is {RequestInfo} or {URL}, makes a request and
* for everything else, calls `WebAssembly.instantiate` directly.
*
* @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
*
* @returns {Promise<InitOutput>}
*/
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;

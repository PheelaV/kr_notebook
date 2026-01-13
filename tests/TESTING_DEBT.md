# Testing Debt

Known gaps in test coverage that should be addressed.

## Lesson Access Control

- [ ] **Locked lessons hidden from dropdown**: Verify that lessons the user hasn't unlocked don't appear in the study/practice filter dropdown
- [ ] **Card selection respects lesson locks**: Verify that even if a locked lesson filter is somehow set, card selection falls back correctly (defense-in-depth is implemented, but no explicit test)

## Content Pack System

- [ ] **Pack installation flow**: Test enabling/disabling packs and verifying cards become available/unavailable
- [ ] **Pack content visibility**: Test that users only see cards from packs they have permission to access
- [ ] **Pack permission inheritance**: Test group-based pack permissions work correctly
- [ ] **Lesson unlock progression**: Test that completing lessons unlocks the next lesson

## Session Stats

- [ ] **Stats match home page**: Verify `new_available + learning_due + reviews_due` equals home page "cards due" count
- [ ] **Stats respect study filter**: Verify stats change when filter is changed

## Daily New Cards Limit

- [ ] **count_new_cards_today accuracy**: Test that cards reviewed multiple times (learning steps) are still counted as 1 new card
- [ ] **Limit enforcement**: Test that new cards stop appearing when daily limit is reached
- [ ] **Limit reset at midnight**: Test that the counter resets at local midnight
- [ ] **Stats bar shows correct count**: Verify "X/Y today" updates correctly as cards are reviewed

## Focus Mode

- [ ] **Focus mode toggle on study page**: Test the lightning button toggles focus mode on/off
- [ ] **Focus mode persists**: Verify focus mode state persists across page reloads
- [ ] **Focus mode and filter are independent**: Changing the study filter should not affect focus mode
- [ ] **Settings sync**: Focus mode toggled on study page should reflect on settings page and vice versa
- [ ] **Focus mode affects learning steps**: When enabled, cards should use faster learning steps (1→5→15→30 min vs 1→10→60→240 min)

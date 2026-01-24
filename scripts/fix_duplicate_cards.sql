-- Fix duplicate cards in app.db
-- Run this on production database after backing up!

-- ============================================================
-- FIX 1: 있다 (itda) - consolidate 3 duplicate cards into 1
-- ============================================================
-- Current state:
--   861: "to have"
--   863: "to be at/in a location"
--   1417: "to be, to exist"
--
-- Solution: Keep 1417, update to consolidated form, delete 861 and 863

-- Update the survivor card with consolidated answer
UPDATE card_definitions
SET main_answer = 'to be, to exist, to have',
    description = '(itda) - verb. Context: existence, location, possession'
WHERE id = 1417;

-- Delete the duplicates (use the exact IDs from your database)
DELETE FROM card_definitions WHERE id IN (861, 863);

-- ============================================================
-- FIX 2: 안 (ahn) - remove verbose duplicate
-- ============================================================
-- Current state:
--   805: "adverb that makes verbs or adjectives negative" (verbose)
--   847: "inside, within" (different word - keep!)
--   1415: "not" (correct short form)
--
-- Solution: Keep 1415, delete 805

DELETE FROM card_definitions WHERE id = 805;

-- ============================================================
-- VERIFY: Check no duplicates remain
-- ============================================================
-- Run these queries to verify:
-- SELECT front, COUNT(*) FROM card_definitions WHERE front IN ('있다', '안') GROUP BY front;
-- Should show: 있다|1 and 안|2 (one for "not", one for "inside")

-- ============================================================
-- CLEANUP: User learning.db files (run separately for each user)
-- ============================================================
-- For each user's learning.db, clean up orphaned card_progress:
--
-- sqlite3 data/users/<username>/learning.db "
-- DELETE FROM card_progress WHERE card_id IN (861, 863, 805);
-- DELETE FROM review_log WHERE card_id IN (861, 863, 805);
-- "
--
-- Or run for all users:
-- for db in data/users/*/learning.db; do
--   sqlite3 "$db" "DELETE FROM card_progress WHERE card_id IN (861, 863, 805);"
--   sqlite3 "$db" "DELETE FROM review_log WHERE card_id IN (861, 863, 805);"
-- done

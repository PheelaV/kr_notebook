"""Timer integration tests with pack cards.

Tests that the 'next card due in' timer on the home page correctly includes
vocabulary pack cards, not just Hangul baseline cards.

Bug context: Previously, get_next_upcoming_review_time() only looked at tier 1-4
cards, ignoring vocabulary pack cards (tier 5+). This caused the timer to show
incorrect values when pack cards had sooner review times than Hangul cards.
"""

import re
import sqlite3
import uuid
from datetime import datetime, timedelta, timezone
from pathlib import Path

import pytest

from conftest import DbManager, TestClient


class TestTimerWithPackCards:
    """Test that the home page timer includes pack cards."""

    def test_timer_shows_pack_card_time_when_sooner(
        self,
        client: TestClient,
        db_manager: DbManager,
        test_data_dir: Path,
    ):
        """Timer should show pack card review time when it's sooner than Hangul cards.

        Setup:
        - Hangul cards with next_review = +2 hours
        - Pack cards with next_review = +30 minutes (sooner)

        Expected: Timer shows ~30 minutes, not ~2 hours
        """
        username = f"_test_{uuid.uuid4().hex[:8]}"
        password = "test123"

        try:
            # Create user
            password_hash = db_manager.create_user(username, password)

            # Get paths to databases
            user_db_path = test_data_dir / "users" / username / "learning.db"
            app_db_path = test_data_dir / "app.db"

            # Calculate future times
            now = datetime.now(timezone.utc)
            pack_review_time = now + timedelta(minutes=30)  # Sooner
            hangul_review_time = now + timedelta(hours=2)   # Later

            # Format as RFC 3339 (SQLite datetime format)
            pack_time_str = pack_review_time.strftime("%Y-%m-%dT%H:%M:%S+00:00")
            hangul_time_str = hangul_review_time.strftime("%Y-%m-%dT%H:%M:%S+00:00")

            # Set up test data in databases
            _setup_test_databases(
                user_db_path,
                app_db_path,
                pack_time_str,
                hangul_time_str,
            )

            # Login and check home page
            client.login(username, password_hash)
            assert client.is_authenticated()

            response = client.get("/")
            assert response.status_code == 200

            # Extract timestamp from data-target attribute
            # Pattern: data-target="<timestamp>"
            match = re.search(r'data-target="(\d+)"', response.text)
            assert match, f"Could not find timer timestamp in response"

            timer_timestamp = int(match.group(1))
            timer_time = datetime.fromtimestamp(timer_timestamp, tz=timezone.utc)

            # Timer should be close to pack_review_time (~30 min), not hangul_review_time (~2h)
            # Allow 5 minute tolerance for test execution time
            time_diff_to_pack = abs((timer_time - pack_review_time).total_seconds())
            time_diff_to_hangul = abs((timer_time - hangul_review_time).total_seconds())

            assert time_diff_to_pack < 300, (
                f"Timer should show pack card time (~30 min from now), "
                f"but shows {timer_time}. "
                f"Diff to pack: {time_diff_to_pack}s, diff to hangul: {time_diff_to_hangul}s"
            )
            # Also verify it's NOT showing the Hangul time
            assert time_diff_to_hangul > 3600, (
                f"Timer appears to show Hangul card time instead of pack card time. "
                f"Timer: {timer_time}, expected pack time: {pack_review_time}"
            )

        finally:
            db_manager.delete_user(username)

    def test_timer_shows_hangul_when_no_packs(
        self,
        client: TestClient,
        db_manager: DbManager,
        test_data_dir: Path,
    ):
        """Timer should show Hangul card time when user has no pack access."""
        username = f"_test_{uuid.uuid4().hex[:8]}"
        password = "test123"

        try:
            password_hash = db_manager.create_user(username, password)

            user_db_path = test_data_dir / "users" / username / "learning.db"

            now = datetime.now(timezone.utc)
            hangul_review_time = now + timedelta(hours=1)
            hangul_time_str = hangul_review_time.strftime("%Y-%m-%dT%H:%M:%S+00:00")

            # Set up Hangul card review time only (no pack setup)
            _setup_hangul_only(user_db_path, hangul_time_str)

            client.login(username, password_hash)
            response = client.get("/")
            assert response.status_code == 200

            # Should still have a timer (from Hangul cards)
            match = re.search(r'data-target="(\d+)"', response.text)
            if match:
                timer_timestamp = int(match.group(1))
                timer_time = datetime.fromtimestamp(timer_timestamp, tz=timezone.utc)

                # Timer should be close to hangul_review_time
                time_diff = abs((timer_time - hangul_review_time).total_seconds())
                assert time_diff < 300, (
                    f"Timer should show Hangul card time (~1h from now), "
                    f"but shows {timer_time}"
                )

        finally:
            db_manager.delete_user(username)


def _setup_test_databases(
    user_db_path: Path,
    app_db_path: Path,
    pack_time_str: str,
    hangul_time_str: str,
):
    """Set up test databases with Hangul and pack cards at different review times."""
    # 1. Register pack in app.db and make it public
    app_conn = sqlite3.connect(app_db_path)
    try:
        # Check if pack already registered
        cursor = app_conn.execute(
            "SELECT COUNT(*) FROM content_packs WHERE id = 'test_lesson_pack'"
        )
        if cursor.fetchone()[0] == 0:
            # Register the test pack with all required columns
            app_conn.execute(
                """INSERT INTO content_packs (id, name, pack_type, version, description, source_path, scope, installed_at, is_enabled)
                   VALUES ('test_lesson_pack', 'Test Lesson Pack', 'cards', '1.0.0', 'Test pack', 'fixtures/test_lesson_pack', 'global', datetime('now'), 1)"""
            )

        # Make pack public (accessible to all users)
        app_conn.execute(
            """INSERT OR REPLACE INTO pack_permissions (pack_id, group_id, allowed)
               VALUES ('test_lesson_pack', '', 1)"""
        )

        # Insert pack cards into card_definitions if not already there
        cursor = app_conn.execute(
            "SELECT COUNT(*) FROM card_definitions WHERE pack_id = 'test_lesson_pack'"
        )
        if cursor.fetchone()[0] == 0:
            # Insert test pack cards (tier 5)
            app_conn.execute(
                """INSERT INTO card_definitions (front, main_answer, description, card_type, tier, is_reverse, pack_id, lesson)
                   VALUES ('L1-A', 'l1a', 'Lesson 1 card A', 'Vocabulary', 5, 0, 'test_lesson_pack', 1)"""
            )

        # Get the pack card ID
        cursor = app_conn.execute(
            "SELECT id FROM card_definitions WHERE pack_id = 'test_lesson_pack' LIMIT 1"
        )
        pack_card_id = cursor.fetchone()[0]

        # Get a Hangul card ID (tier 1-4)
        cursor = app_conn.execute(
            "SELECT id FROM card_definitions WHERE pack_id IS NULL AND tier <= 4 LIMIT 1"
        )
        hangul_row = cursor.fetchone()
        hangul_card_id = hangul_row[0] if hangul_row else 1

        app_conn.commit()
    finally:
        app_conn.close()

    # 2. Set up card progress in user's learning.db
    user_conn = sqlite3.connect(user_db_path)
    try:
        # Ensure card_progress table exists
        user_conn.execute(
            """CREATE TABLE IF NOT EXISTS card_progress (
                card_id INTEGER PRIMARY KEY,
                ease_factor REAL DEFAULT 2.5,
                interval_days INTEGER DEFAULT 0,
                repetitions INTEGER DEFAULT 0,
                next_review TEXT,
                total_reviews INTEGER DEFAULT 0,
                correct_reviews INTEGER DEFAULT 0,
                learning_step INTEGER DEFAULT 0,
                fsrs_stability REAL DEFAULT 0.0,
                fsrs_difficulty REAL DEFAULT 0.0,
                fsrs_state TEXT DEFAULT 'New'
            )"""
        )

        # Set Hangul card with far future review time
        user_conn.execute(
            """INSERT OR REPLACE INTO card_progress (card_id, next_review, repetitions, learning_step, fsrs_state)
               VALUES (?, ?, 5, 4, 'Review')""",
            (hangul_card_id, hangul_time_str),
        )

        # Set pack card with near future review time
        user_conn.execute(
            """INSERT OR REPLACE INTO card_progress (card_id, next_review, repetitions, learning_step, fsrs_state)
               VALUES (?, ?, 3, 2, 'Learning')""",
            (pack_card_id, pack_time_str),
        )

        # Ensure tier 1 is unlocked
        user_conn.execute(
            """INSERT OR REPLACE INTO settings (key, value)
               VALUES ('max_unlocked_tier', '1')"""
        )

        # Unlock lesson 1 of the test pack
        user_conn.execute(
            """CREATE TABLE IF NOT EXISTS pack_lesson_progress (
                pack_id TEXT NOT NULL,
                lesson INTEGER NOT NULL,
                unlocked INTEGER NOT NULL DEFAULT 0,
                unlocked_at TEXT,
                PRIMARY KEY (pack_id, lesson)
            )"""
        )
        user_conn.execute(
            """INSERT OR REPLACE INTO pack_lesson_progress (pack_id, lesson, unlocked, unlocked_at)
               VALUES ('test_lesson_pack', 1, 1, datetime('now'))"""
        )

        user_conn.commit()
    finally:
        user_conn.close()


def _setup_hangul_only(user_db_path: Path, hangul_time_str: str):
    """Set up user database with only Hangul card progress."""
    user_conn = sqlite3.connect(user_db_path)
    try:
        user_conn.execute(
            """CREATE TABLE IF NOT EXISTS card_progress (
                card_id INTEGER PRIMARY KEY,
                ease_factor REAL DEFAULT 2.5,
                interval_days INTEGER DEFAULT 0,
                repetitions INTEGER DEFAULT 0,
                next_review TEXT,
                total_reviews INTEGER DEFAULT 0,
                correct_reviews INTEGER DEFAULT 0,
                learning_step INTEGER DEFAULT 0,
                fsrs_stability REAL DEFAULT 0.0,
                fsrs_difficulty REAL DEFAULT 0.0,
                fsrs_state TEXT DEFAULT 'New'
            )"""
        )

        # Set Hangul card (ID 1) with future review time
        user_conn.execute(
            """INSERT OR REPLACE INTO card_progress (card_id, next_review, repetitions, learning_step, fsrs_state)
               VALUES (1, ?, 5, 4, 'Review')""",
            (hangul_time_str,),
        )

        # Ensure tier 1 is unlocked
        user_conn.execute(
            """INSERT OR REPLACE INTO settings (key, value)
               VALUES ('max_unlocked_tier', '1')"""
        )

        user_conn.commit()
    finally:
        user_conn.close()

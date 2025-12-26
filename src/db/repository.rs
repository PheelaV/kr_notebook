use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result};

use crate::domain::{Card, CardType, FsrsState, ReviewDirection, ReviewLog, StudyMode};
#[cfg(feature = "profiling")]
use crate::profiling::EventType;

pub fn insert_card(conn: &Connection, card: &Card) -> Result<i64> {
  conn.execute(
    r#"
    INSERT INTO cards (front, main_answer, description, card_type, tier, audio_hint, ease_factor,
                       interval_days, repetitions, next_review, total_reviews, correct_reviews)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
    "#,
    params![
      card.front,
      card.main_answer,
      card.description,
      card.card_type.as_str(),
      card.tier,
      card.audio_hint,
      card.ease_factor,
      card.interval_days,
      card.repetitions,
      card.next_review.to_rfc3339(),
      card.total_reviews,
      card.correct_reviews,
    ],
  )?;
  Ok(conn.last_insert_rowid())
}

pub fn get_card_by_id(conn: &Connection, id: i64) -> Result<Option<Card>> {
  let mut stmt = conn.prepare(
    r#"
    SELECT id, front, main_answer, description, card_type, tier, audio_hint, ease_factor,
           interval_days, repetitions, next_review, total_reviews, correct_reviews, learning_step,
           fsrs_stability, fsrs_difficulty, fsrs_state
    FROM cards WHERE id = ?1
    "#,
  )?;

  let mut rows = stmt.query(params![id])?;
  if let Some(row) = rows.next()? {
    Ok(Some(row_to_card(row)?))
  } else {
    Ok(None)
  }
}

pub fn get_due_cards(conn: &Connection, limit: usize, exclude_sibling_of: Option<i64>) -> Result<Vec<Card>> {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::DbQuery {
    operation: "select".into(),
    table: "cards".into(),
  });

  let now = Utc::now().to_rfc3339();
  let effective_tiers = get_effective_tiers(conn)?;

  if effective_tiers.is_empty() {
    return Ok(vec![]);
  }

  // Build tier list for IN clause (safe: only u8 integers)
  let tier_list = effective_tiers
    .iter()
    .map(|t| t.to_string())
    .collect::<Vec<_>>()
    .join(",");

  // If we have a last card, exclude its sibling (reverse card)
  if let Some(last_id) = exclude_sibling_of {
    // Get the last card's front and main_answer to identify siblings
    if let Ok(Some(last_card)) = get_card_by_id(conn, last_id) {
      let query = format!(
        r#"
        SELECT id, front, main_answer, description, card_type, tier, audio_hint, ease_factor,
               interval_days, repetitions, next_review, total_reviews, correct_reviews, learning_step,
               fsrs_stability, fsrs_difficulty, fsrs_state
        FROM cards
        WHERE next_review <= ?1 AND tier IN ({})
          AND id != ?2
          AND main_answer != ?3
          AND front NOT LIKE '%' || ?4 || '%'
        ORDER BY tier ASC, next_review ASC
        LIMIT ?5
        "#,
        tier_list
      );
      let mut stmt = conn.prepare(&query)?;

      let cards = stmt
        .query_map(
          params![now, last_id, last_card.front, last_card.main_answer, limit as i64],
          |row| row_to_card(row),
        )?
        .collect::<Result<Vec<_>>>()?;
      return Ok(cards);
    }
  }

  // Default: no sibling exclusion
  let query = format!(
    r#"
    SELECT id, front, main_answer, description, card_type, tier, audio_hint, ease_factor,
           interval_days, repetitions, next_review, total_reviews, correct_reviews, learning_step,
           fsrs_stability, fsrs_difficulty, fsrs_state
    FROM cards
    WHERE next_review <= ?1 AND tier IN ({})
    ORDER BY tier ASC, next_review ASC
    LIMIT ?2
    "#,
    tier_list
  );
  let mut stmt = conn.prepare(&query)?;

  let cards = stmt
    .query_map(params![now, limit as i64], |row| row_to_card(row))?
    .collect::<Result<Vec<_>>>()?;

  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::DbQueryComplete {
    operation: "get_due_cards".into(),
    rows: cards.len() as i64,
  });

  Ok(cards)
}

pub fn get_due_count(conn: &Connection) -> Result<i64> {
  let now = Utc::now().to_rfc3339();
  let effective_tiers = get_effective_tiers(conn)?;

  if effective_tiers.is_empty() {
    return Ok(0);
  }

  let tier_list = effective_tiers
    .iter()
    .map(|t| t.to_string())
    .collect::<Vec<_>>()
    .join(",");

  let query = format!(
    "SELECT COUNT(*) FROM cards WHERE next_review <= ?1 AND tier IN ({})",
    tier_list
  );
  conn.query_row(&query, params![now], |row| row.get(0))
}

/// Get the next scheduled review time (for cards not yet due)
pub fn get_next_review_time(conn: &Connection) -> Result<Option<DateTime<Utc>>> {
  let now = Utc::now().to_rfc3339();
  let effective_tiers = get_effective_tiers(conn)?;

  if effective_tiers.is_empty() {
    return Ok(None);
  }

  let tier_list = effective_tiers
    .iter()
    .map(|t| t.to_string())
    .collect::<Vec<_>>()
    .join(",");

  let query = format!(
    "SELECT next_review FROM cards WHERE next_review > ?1 AND tier IN ({}) ORDER BY next_review ASC LIMIT 1",
    tier_list
  );

  let result: std::result::Result<String, _> = conn.query_row(&query, params![now], |row| row.get(0));

  match result {
    Ok(next_review_str) => {
      let dt = DateTime::parse_from_rfc3339(&next_review_str)
        .map(|dt| dt.with_timezone(&Utc))
        .ok();
      Ok(dt)
    }
    Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
    Err(e) => Err(e),
  }
}

/// Get due cards with interleaving (mix card types for better learning)
/// Returns cards interleaved by card_type to avoid clustering similar cards
pub fn get_due_cards_interleaved(conn: &Connection, limit: usize, exclude_sibling_of: Option<i64>) -> Result<Vec<Card>> {
  // First get all due cards
  let mut cards = get_due_cards(conn, limit * 3, exclude_sibling_of)?;

  if cards.len() <= 1 {
    return Ok(cards.into_iter().take(limit).collect());
  }

  // Group by card_type
  let mut by_type: std::collections::HashMap<String, Vec<Card>> = std::collections::HashMap::new();
  for card in cards.drain(..) {
    by_type
      .entry(card.card_type.as_str().to_string())
      .or_default()
      .push(card);
  }

  // Interleave: take one from each type in round-robin
  let mut result = Vec::with_capacity(limit);
  let mut type_iters: Vec<_> = by_type.into_values().map(|v| v.into_iter()).collect();

  'outer: loop {
    let mut made_progress = false;
    for iter in &mut type_iters {
      if let Some(card) = iter.next() {
        result.push(card);
        made_progress = true;
        if result.len() >= limit {
          break 'outer;
        }
      }
    }
    if !made_progress {
      break;
    }
  }

  Ok(result)
}

/// Get cards for practice mode - any unlocked card, ordered by least recently reviewed
pub fn get_practice_cards(conn: &Connection, limit: usize, exclude_id: Option<i64>) -> Result<Vec<Card>> {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::DbQuery {
    operation: "select".into(),
    table: "cards".into(),
  });

  let effective_tiers = get_effective_tiers(conn)?;

  if effective_tiers.is_empty() {
    return Ok(vec![]);
  }

  let tier_list = effective_tiers
    .iter()
    .map(|t| t.to_string())
    .collect::<Vec<_>>()
    .join(",");

  if let Some(last_id) = exclude_id {
    if let Ok(Some(last_card)) = get_card_by_id(conn, last_id) {
      let query = format!(
        r#"
        SELECT id, front, main_answer, description, card_type, tier, audio_hint, ease_factor,
               interval_days, repetitions, next_review, total_reviews, correct_reviews, learning_step,
               fsrs_stability, fsrs_difficulty, fsrs_state
        FROM cards
        WHERE tier IN ({})
          AND id != ?1
          AND main_answer != ?2
          AND front NOT LIKE '%' || ?3 || '%'
        ORDER BY RANDOM()
        LIMIT ?4
        "#,
        tier_list
      );
      let mut stmt = conn.prepare(&query)?;

      let cards = stmt
        .query_map(
          params![last_id, last_card.front, last_card.main_answer, limit as i64],
          |row| row_to_card(row),
        )?
        .collect::<Result<Vec<_>>>()?;
      return Ok(cards);
    }
  }

  let query = format!(
    r#"
    SELECT id, front, main_answer, description, card_type, tier, audio_hint, ease_factor,
           interval_days, repetitions, next_review, total_reviews, correct_reviews, learning_step,
           fsrs_stability, fsrs_difficulty, fsrs_state
    FROM cards
    WHERE tier IN ({})
    ORDER BY RANDOM()
    LIMIT ?1
    "#,
    tier_list
  );
  let mut stmt = conn.prepare(&query)?;

  let cards = stmt
    .query_map(params![limit as i64], |row| row_to_card(row))?
    .collect::<Result<Vec<_>>>()?;

  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::DbQueryComplete {
    operation: "get_practice_cards".into(),
    rows: cards.len() as i64,
  });

  Ok(cards)
}

/// Get all unlocked cards, ordered by tier and then by front
pub fn get_unlocked_cards(conn: &Connection) -> Result<Vec<Card>> {
  let effective_tiers = get_effective_tiers(conn)?;

  if effective_tiers.is_empty() {
    return Ok(vec![]);
  }

  let tier_list = effective_tiers
    .iter()
    .map(|t| t.to_string())
    .collect::<Vec<_>>()
    .join(",");

  let query = format!(
    r#"
    SELECT id, front, main_answer, description, card_type, tier, audio_hint, ease_factor,
           interval_days, repetitions, next_review, total_reviews, correct_reviews, learning_step,
           fsrs_stability, fsrs_difficulty, fsrs_state
    FROM cards
    WHERE tier IN ({})
    ORDER BY tier ASC, front ASC
    "#,
    tier_list
  );
  let mut stmt = conn.prepare(&query)?;

  let cards = stmt
    .query_map([], |row| row_to_card(row))?
    .collect::<Result<Vec<_>>>()?;
  Ok(cards)
}

/// Get cards not yet reviewed today (for accelerated mode)
/// Returns cards from enabled tiers that haven't been reviewed in today's session
/// Excludes cards that are already due (those are handled by get_due_cards)
pub fn get_unreviewed_today(conn: &Connection, limit: usize, exclude_sibling_of: Option<i64>) -> Result<Vec<Card>> {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::DbQuery {
    operation: "select".into(),
    table: "cards".into(),
  });

  let now = Utc::now().to_rfc3339();
  let effective_tiers = get_effective_tiers(conn)?;

  if effective_tiers.is_empty() {
    return Ok(vec![]);
  }

  let tier_list = effective_tiers
    .iter()
    .map(|t| t.to_string())
    .collect::<Vec<_>>()
    .join(",");

  // Get cards that haven't been reviewed today and are not due
  if let Some(last_id) = exclude_sibling_of {
    if let Ok(Some(last_card)) = get_card_by_id(conn, last_id) {
      let query = format!(
        r#"
        SELECT c.id, c.front, c.main_answer, c.description, c.card_type, c.tier, c.audio_hint,
               c.ease_factor, c.interval_days, c.repetitions, c.next_review, c.total_reviews,
               c.correct_reviews, c.learning_step, c.fsrs_stability, c.fsrs_difficulty, c.fsrs_state
        FROM cards c
        WHERE c.tier IN ({})
          AND c.next_review > ?1
          AND c.id != ?2
          AND c.main_answer != ?3
          AND c.front NOT LIKE '%' || ?4 || '%'
          AND c.id NOT IN (
            SELECT DISTINCT card_id FROM review_logs
            WHERE date(reviewed_at) = date('now')
          )
        ORDER BY c.tier ASC, c.id ASC
        LIMIT ?5
        "#,
        tier_list
      );
      let mut stmt = conn.prepare(&query)?;
      let cards = stmt
        .query_map(
          params![now, last_id, last_card.front, last_card.main_answer, limit as i64],
          |row| row_to_card(row),
        )?
        .collect::<Result<Vec<_>>>()?;
      return Ok(cards);
    }
  }

  let query = format!(
    r#"
    SELECT c.id, c.front, c.main_answer, c.description, c.card_type, c.tier, c.audio_hint,
           c.ease_factor, c.interval_days, c.repetitions, c.next_review, c.total_reviews,
           c.correct_reviews, c.learning_step, c.fsrs_stability, c.fsrs_difficulty, c.fsrs_state
    FROM cards c
    WHERE c.tier IN ({})
      AND c.next_review > ?1
      AND c.id NOT IN (
        SELECT DISTINCT card_id FROM review_logs
        WHERE date(reviewed_at) = date('now')
      )
    ORDER BY c.tier ASC, c.id ASC
    LIMIT ?2
    "#,
    tier_list
  );
  let mut stmt = conn.prepare(&query)?;
  let cards = stmt
    .query_map(params![now, limit as i64], |row| row_to_card(row))?
    .collect::<Result<Vec<_>>>()?;

  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::DbQueryComplete {
    operation: "get_unreviewed_today".into(),
    rows: cards.len() as i64,
  });

  Ok(cards)
}

/// Count cards not yet reviewed today (for accelerated mode display)
/// Excludes cards that are already due (to avoid double-counting with get_due_count)
pub fn get_unreviewed_today_count(conn: &Connection) -> Result<i64> {
  let now = Utc::now().to_rfc3339();
  let effective_tiers = get_effective_tiers(conn)?;

  if effective_tiers.is_empty() {
    return Ok(0);
  }

  let tier_list = effective_tiers
    .iter()
    .map(|t| t.to_string())
    .collect::<Vec<_>>()
    .join(",");

  let query = format!(
    r#"
    SELECT COUNT(*) FROM cards c
    WHERE c.tier IN ({})
      AND c.next_review > ?1
      AND c.id NOT IN (
        SELECT DISTINCT card_id FROM review_logs
        WHERE date(reviewed_at) = date('now')
      )
    "#,
    tier_list
  );
  conn.query_row(&query, params![now], |row| row.get(0))
}

/// Get all cards for a specific tier (for generating multiple choice options)
pub fn get_cards_by_tier(conn: &Connection, tier: u8) -> Result<Vec<Card>> {
  let mut stmt = conn.prepare(
    r#"
    SELECT id, front, main_answer, description, card_type, tier, audio_hint, ease_factor,
           interval_days, repetitions, next_review, total_reviews, correct_reviews, learning_step,
           fsrs_stability, fsrs_difficulty, fsrs_state
    FROM cards
    WHERE tier = ?1
    ORDER BY id ASC
    "#,
  )?;

  let cards = stmt
    .query_map(params![tier], |row| row_to_card(row))?
    .collect::<Result<Vec<_>>>()?;
  Ok(cards)
}

pub fn update_card_after_review(
  conn: &Connection,
  id: i64,
  ease_factor: f64,
  interval_days: i64,
  repetitions: i64,
  next_review: DateTime<Utc>,
  learning_step: i64,
  correct: bool,
) -> Result<()> {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::DbQuery {
    operation: "update".into(),
    table: "cards".into(),
  });

  conn.execute(
    r#"
    UPDATE cards
    SET ease_factor = ?1, interval_days = ?2, repetitions = ?3, next_review = ?4,
        learning_step = ?5, total_reviews = total_reviews + 1,
        correct_reviews = correct_reviews + ?6
    WHERE id = ?7
    "#,
    params![
      ease_factor,
      interval_days,
      repetitions,
      next_review.to_rfc3339(),
      learning_step,
      if correct { 1 } else { 0 },
      id,
    ],
  )?;
  Ok(())
}

/// Update card with FSRS scheduling data
pub fn update_card_after_fsrs_review(
  conn: &Connection,
  id: i64,
  next_review: DateTime<Utc>,
  stability: f64,
  difficulty: f64,
  state: FsrsState,
  correct: bool,
) -> Result<()> {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::DbQuery {
    operation: "update_fsrs".into(),
    table: "cards".into(),
  });

  conn.execute(
    r#"
    UPDATE cards
    SET next_review = ?1, fsrs_stability = ?2, fsrs_difficulty = ?3, fsrs_state = ?4,
        total_reviews = total_reviews + 1, correct_reviews = correct_reviews + ?5,
        repetitions = CASE WHEN ?5 = 1 THEN repetitions + 1 ELSE 0 END
    WHERE id = ?6
    "#,
    params![
      next_review.to_rfc3339(),
      stability,
      difficulty,
      state.as_str(),
      if correct { 1 } else { 0 },
      id,
    ],
  )?;
  Ok(())
}

// Confusion tracking functions

/// Record a wrong answer for a card
pub fn record_confusion(conn: &Connection, card_id: i64, wrong_answer: &str) -> Result<()> {
  let now = Utc::now().to_rfc3339();

  // Try to update existing confusion entry first
  let updated = conn.execute(
    r#"
    UPDATE confusions
    SET count = count + 1, last_confused_at = ?1
    WHERE card_id = ?2 AND wrong_answer = ?3
    "#,
    params![now, card_id, wrong_answer],
  )?;

  // If no existing entry, insert new one
  if updated == 0 {
    conn.execute(
      r#"
      INSERT INTO confusions (card_id, wrong_answer, count, last_confused_at)
      VALUES (?1, ?2, 1, ?3)
      "#,
      params![card_id, wrong_answer, now],
    )?;
  }

  Ok(())
}

/// Get top confusions for a card (most frequently confused answers)
pub fn get_card_confusions(conn: &Connection, card_id: i64, limit: usize) -> Result<Vec<(String, i64)>> {
  let mut stmt = conn.prepare(
    r#"
    SELECT wrong_answer, count
    FROM confusions
    WHERE card_id = ?1
    ORDER BY count DESC
    LIMIT ?2
    "#,
  )?;

  let confusions = stmt
    .query_map(params![card_id, limit as i64], |row| {
      Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?
    .collect::<Result<Vec<_>>>()?;

  Ok(confusions)
}

/// Get cards with most confusions (problem areas)
pub fn get_problem_cards(conn: &Connection, limit: usize) -> Result<Vec<(i64, String, i64)>> {
  let mut stmt = conn.prepare(
    r#"
    SELECT c.id, c.front, SUM(cf.count) as total_confusions
    FROM cards c
    JOIN confusions cf ON c.id = cf.card_id
    GROUP BY c.id
    ORDER BY total_confusions DESC
    LIMIT ?1
    "#,
  )?;

  let problems = stmt
    .query_map(params![limit as i64], |row| {
      Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?))
    })?
    .collect::<Result<Vec<_>>>()?;

  Ok(problems)
}

/// Check if FSRS is enabled in settings
pub fn get_use_fsrs(conn: &Connection) -> Result<bool> {
  get_setting(conn, "use_fsrs").map(|v| v.as_deref() != Some("false"))
}

/// Get desired retention target (default 0.9 = 90%)
pub fn get_desired_retention(conn: &Connection) -> Result<f64> {
  get_setting(conn, "desired_retention")
    .map(|v| v.and_then(|s| s.parse().ok()).unwrap_or(0.9))
}

/// Check if interleaving is enabled (mixing card types)
pub fn get_use_interleaving(conn: &Connection) -> Result<bool> {
  get_setting(conn, "use_interleaving").map(|v| v.as_deref() != Some("false"))
}

pub fn insert_review_log(conn: &Connection, log: &ReviewLog) -> Result<i64> {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::DbQuery {
    operation: "insert".into(),
    table: "review_logs".into(),
  });

  conn.execute(
    "INSERT INTO review_logs (card_id, quality, reviewed_at) VALUES (?1, ?2, ?3)",
    params![log.card_id, log.quality, log.reviewed_at.to_rfc3339()],
  )?;
  Ok(conn.last_insert_rowid())
}

/// Insert a review log with enhanced tracking fields
pub fn insert_review_log_enhanced(
  conn: &Connection,
  card_id: i64,
  quality: u8,
  is_correct: bool,
  study_mode: StudyMode,
  direction: ReviewDirection,
  response_time_ms: Option<i64>,
  hints_used: i32,
) -> Result<i64> {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::DbQuery {
    operation: "insert_enhanced".into(),
    table: "review_logs".into(),
  });

  let now = Utc::now().to_rfc3339();
  conn.execute(
    r#"
    INSERT INTO review_logs (card_id, quality, reviewed_at, is_correct, study_mode, direction, response_time_ms, hints_used)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
    "#,
    params![
      card_id,
      quality,
      now,
      if is_correct { 1 } else { 0 },
      study_mode.as_str(),
      direction.as_str(),
      response_time_ms,
      hints_used,
    ],
  )?;
  Ok(conn.last_insert_rowid())
}

// Settings functions

pub fn get_max_unlocked_tier(conn: &Connection) -> Result<u8> {
  // If all tiers are unlocked, return max tier (4)
  if get_all_tiers_unlocked(conn).unwrap_or(false) {
    return Ok(4);
  }

  conn.query_row(
    "SELECT value FROM settings WHERE key = 'max_unlocked_tier'",
    [],
    |row| {
      let val: String = row.get(0)?;
      Ok(val.parse::<u8>().unwrap_or(1))
    },
  )
}

/// Get the effective list of tiers to show cards from.
/// This considers max_unlocked_tier, enabled_tiers, and all_tiers_unlocked settings.
pub fn get_effective_tiers(conn: &Connection) -> Result<Vec<u8>> {
  let all_unlocked = get_all_tiers_unlocked(conn)?;
  let enabled_tiers = get_enabled_tiers(conn)?;

  if all_unlocked {
    // When accelerated mode is on, use all enabled tiers
    Ok(enabled_tiers)
  } else {
    // Normal mode: intersection of unlocked AND enabled
    let max_tier = get_max_unlocked_tier(conn)?;
    Ok(
      enabled_tiers
        .into_iter()
        .filter(|&t| t <= max_tier)
        .collect(),
    )
  }
}

pub fn set_max_unlocked_tier(conn: &Connection, tier: u8) -> Result<()> {
  conn.execute(
    "UPDATE settings SET value = ?1 WHERE key = 'max_unlocked_tier'",
    params![tier.to_string()],
  )?;
  Ok(())
}

pub fn unlock_next_tier(conn: &Connection) -> Result<u8> {
  let current = get_max_unlocked_tier(conn)?;
  let next = (current + 1).min(4);
  set_max_unlocked_tier(conn, next)?;
  Ok(next)
}

// Generic settings functions

pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
  let result: std::result::Result<String, _> = conn.query_row(
    "SELECT value FROM settings WHERE key = ?1",
    params![key],
    |row| row.get(0),
  );

  match result {
    Ok(value) => Ok(Some(value)),
    Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
    Err(e) => Err(e),
  }
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
  conn.execute(
    "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
    params![key, value],
  )?;
  Ok(())
}

// Dark mode settings (for future use)

#[allow(dead_code)]
pub fn get_dark_mode(conn: &Connection) -> Result<bool> {
  get_setting(conn, "dark_mode").map(|v| v.as_deref() == Some("true"))
}

#[allow(dead_code)]
pub fn set_dark_mode(conn: &Connection, enabled: bool) -> Result<()> {
  set_setting(conn, "dark_mode", if enabled { "true" } else { "false" })
}

// TTS settings (for future use)

#[allow(dead_code)]
pub fn get_tts_enabled(conn: &Connection) -> Result<bool> {
  get_setting(conn, "tts_enabled").map(|v| v.as_deref() != Some("false"))
}

#[allow(dead_code)]
pub fn set_tts_enabled(conn: &Connection, enabled: bool) -> Result<()> {
  set_setting(conn, "tts_enabled", if enabled { "true" } else { "false" })
}

#[allow(dead_code)]
pub fn get_tts_model(conn: &Connection) -> Result<String> {
  get_setting(conn, "tts_model").map(|v| v.unwrap_or_else(|| "mms".to_string()))
}

#[allow(dead_code)]
pub fn set_tts_model(conn: &Connection, model: &str) -> Result<()> {
  set_setting(conn, "tts_model", model)
}

// All tiers unlocked setting (escape hatch for accelerated learning)

pub fn get_all_tiers_unlocked(conn: &Connection) -> Result<bool> {
  get_setting(conn, "all_tiers_unlocked").map(|v| v.as_deref() == Some("true"))
}

pub fn set_all_tiers_unlocked(conn: &Connection, enabled: bool) -> Result<()> {
  set_setting(conn, "all_tiers_unlocked", if enabled { "true" } else { "false" })
}

// Per-tier enable/disable (for fine-grained control when all_tiers_unlocked is true)

pub fn get_enabled_tiers(conn: &Connection) -> Result<Vec<u8>> {
  let value = get_setting(conn, "enabled_tiers")?.unwrap_or_else(|| "1,2,3,4".to_string());
  Ok(
    value
      .split(',')
      .filter_map(|s| s.trim().parse::<u8>().ok())
      .collect(),
  )
}

pub fn set_enabled_tiers(conn: &Connection, tiers: &[u8]) -> Result<()> {
  let value = tiers
    .iter()
    .map(|t| t.to_string())
    .collect::<Vec<_>>()
    .join(",");
  set_setting(conn, "enabled_tiers", &value)
}

#[allow(dead_code)]
pub fn is_tier_enabled(conn: &Connection, tier: u8) -> Result<bool> {
  let enabled = get_enabled_tiers(conn)?;
  Ok(enabled.contains(&tier))
}

#[derive(Debug, Clone)]
pub struct TierProgress {
  pub tier: u8,
  pub total: i64,
  pub new_cards: i64,
  pub learning: i64,
  pub learned: i64,
  pub is_unlocked: bool,
  pub is_enabled: bool,
}

impl TierProgress {
  pub fn percentage(&self) -> i64 {
    if self.total > 0 {
      (self.learned * 100) / self.total
    } else {
      0
    }
  }
}

pub fn get_progress_by_tier(conn: &Connection) -> Result<Vec<TierProgress>> {
  let max_tier = get_max_unlocked_tier(conn)?;
  let enabled_tiers = get_enabled_tiers(conn)?;

  let mut stmt = conn.prepare(
    r#"
    SELECT tier,
           COUNT(*) as total,
           SUM(CASE WHEN repetitions = 0 THEN 1 ELSE 0 END) as new_cards,
           SUM(CASE WHEN repetitions > 0 AND repetitions < 2 THEN 1 ELSE 0 END) as learning,
           SUM(CASE WHEN repetitions >= 2 THEN 1 ELSE 0 END) as learned
    FROM cards
    GROUP BY tier
    ORDER BY tier
    "#,
  )?;

  let progress = stmt
    .query_map([], |row| {
      let tier: u8 = row.get(0)?;
      Ok(TierProgress {
        tier,
        total: row.get(1)?,
        new_cards: row.get(2)?,
        learning: row.get(3)?,
        learned: row.get(4)?,
        is_unlocked: tier <= max_tier,
        is_enabled: enabled_tiers.contains(&tier),
      })
    })?
    .collect::<Result<Vec<_>>>()?;
  Ok(progress)
}

/// Make all cards due now (for testing/accelerated learning)
pub fn make_all_cards_due(conn: &Connection) -> Result<usize> {
  let now = Utc::now().to_rfc3339();
  let count = conn.execute(
    "UPDATE cards SET next_review = ?1 WHERE next_review > ?1",
    params![now],
  )?;
  Ok(count)
}

pub fn get_total_stats(conn: &Connection) -> Result<(i64, i64, i64)> {
  let effective_tiers = get_effective_tiers(conn)?;

  if effective_tiers.is_empty() {
    return Ok((0, 0, 0));
  }

  let tier_list = effective_tiers
    .iter()
    .map(|t| t.to_string())
    .collect::<Vec<_>>()
    .join(",");

  let total_cards: i64 = conn.query_row(
    &format!("SELECT COUNT(*) FROM cards WHERE tier IN ({})", tier_list),
    [],
    |row| row.get(0),
  )?;
  let total_reviews: i64 = conn.query_row(
    &format!(
      "SELECT COALESCE(SUM(total_reviews), 0) FROM cards WHERE tier IN ({})",
      tier_list
    ),
    [],
    |row| row.get(0),
  )?;
  let cards_learned: i64 = conn.query_row(
    &format!(
      "SELECT COUNT(*) FROM cards WHERE repetitions >= 2 AND tier IN ({})",
      tier_list
    ),
    [],
    |row| row.get(0),
  )?;
  Ok((total_cards, total_reviews, cards_learned))
}

fn row_to_card(row: &rusqlite::Row) -> Result<Card> {
  let card_type_str: String = row.get(4)?;
  let next_review_str: String = row.get(10)?;
  let fsrs_state_str: Option<String> = row.get(16)?;

  Ok(Card {
    id: row.get(0)?,
    front: row.get(1)?,
    main_answer: row.get(2)?,
    description: row.get(3)?,
    card_type: CardType::from_str(&card_type_str).unwrap_or(CardType::Consonant),
    tier: row.get(5)?,
    audio_hint: row.get(6)?,
    ease_factor: row.get(7)?,
    interval_days: row.get(8)?,
    repetitions: row.get(9)?,
    next_review: DateTime::parse_from_rfc3339(&next_review_str)
      .map(|dt| dt.with_timezone(&Utc))
      .unwrap_or_else(|_| Utc::now()),
    total_reviews: row.get(11)?,
    correct_reviews: row.get(12)?,
    learning_step: row.get(13)?,
    fsrs_stability: row.get(14)?,
    fsrs_difficulty: row.get(15)?,
    fsrs_state: fsrs_state_str.map(|s| FsrsState::from_str(&s)),
  })
}

// ==================== Character Stats ====================

#[derive(Debug, Clone)]
pub struct CharacterStats {
  pub character: String,
  pub character_type: String,
  pub total_attempts: i64,
  pub total_correct: i64,
  pub attempts_7d: i64,
  pub correct_7d: i64,
  pub attempts_1d: i64,
  pub correct_1d: i64,
  pub last_attempt_at: Option<DateTime<Utc>>,
}

impl CharacterStats {
  pub fn lifetime_rate(&self) -> f64 {
    if self.total_attempts > 0 {
      self.total_correct as f64 / self.total_attempts as f64
    } else {
      0.0
    }
  }

  pub fn rate_7d(&self) -> f64 {
    if self.attempts_7d > 0 {
      self.correct_7d as f64 / self.attempts_7d as f64
    } else {
      0.0
    }
  }

  pub fn rate_1d(&self) -> f64 {
    if self.attempts_1d > 0 {
      self.correct_1d as f64 / self.attempts_1d as f64
    } else {
      0.0
    }
  }
}

/// Update character stats after a review
/// This updates both the cached stats table and handles decay windows
pub fn update_character_stats(
  conn: &Connection,
  character: &str,
  character_type: &str,
  is_correct: bool,
) -> Result<()> {
  let now = Utc::now().to_rfc3339();

  // Try to update existing row first
  let updated = conn.execute(
    r#"
    UPDATE character_stats
    SET total_attempts = total_attempts + 1,
        total_correct = total_correct + ?1,
        attempts_7d = attempts_7d + 1,
        correct_7d = correct_7d + ?1,
        attempts_1d = attempts_1d + 1,
        correct_1d = correct_1d + ?1,
        last_attempt_at = ?2
    WHERE character = ?3
    "#,
    params![if is_correct { 1 } else { 0 }, now, character],
  )?;

  // If no existing row, insert new one
  if updated == 0 {
    conn.execute(
      r#"
      INSERT INTO character_stats (character, character_type, total_attempts, total_correct,
                                   attempts_7d, correct_7d, attempts_1d, correct_1d, last_attempt_at)
      VALUES (?1, ?2, 1, ?3, 1, ?3, 1, ?3, ?4)
      "#,
      params![
        character,
        character_type,
        if is_correct { 1 } else { 0 },
        now
      ],
    )?;
  }

  Ok(())
}

/// Get character stats for a specific character
pub fn get_character_stats(conn: &Connection, character: &str) -> Result<Option<CharacterStats>> {
  let mut stmt = conn.prepare(
    r#"
    SELECT character, character_type, total_attempts, total_correct,
           attempts_7d, correct_7d, attempts_1d, correct_1d, last_attempt_at
    FROM character_stats
    WHERE character = ?1
    "#,
  )?;

  let mut rows = stmt.query(params![character])?;
  if let Some(row) = rows.next()? {
    Ok(Some(row_to_character_stats(row)?))
  } else {
    Ok(None)
  }
}

/// Get all character stats for a given type (consonant, vowel, syllable)
pub fn get_character_stats_by_type(conn: &Connection, character_type: &str) -> Result<Vec<CharacterStats>> {
  let mut stmt = conn.prepare(
    r#"
    SELECT character, character_type, total_attempts, total_correct,
           attempts_7d, correct_7d, attempts_1d, correct_1d, last_attempt_at
    FROM character_stats
    WHERE character_type = ?1
    ORDER BY character ASC
    "#,
  )?;

  let stats = stmt
    .query_map(params![character_type], |row| row_to_character_stats(row))?
    .collect::<Result<Vec<_>>>()?;
  Ok(stats)
}

/// Get all character stats ordered by type and character
pub fn get_all_character_stats(conn: &Connection) -> Result<Vec<CharacterStats>> {
  let mut stmt = conn.prepare(
    r#"
    SELECT character, character_type, total_attempts, total_correct,
           attempts_7d, correct_7d, attempts_1d, correct_1d, last_attempt_at
    FROM character_stats
    ORDER BY
      CASE character_type
        WHEN 'consonant' THEN 1
        WHEN 'vowel' THEN 2
        WHEN 'aspirated_consonant' THEN 3
        WHEN 'tense_consonant' THEN 4
        WHEN 'compound_vowel' THEN 5
        ELSE 6
      END,
      character ASC
    "#,
  )?;

  let stats = stmt
    .query_map([], |row| row_to_character_stats(row))?
    .collect::<Result<Vec<_>>>()?;
  Ok(stats)
}

/// Refresh decay windows for all character stats
/// Call this periodically (e.g., on app start or daily) to recalculate 7d/1d windows
pub fn refresh_character_stats_decay(conn: &Connection) -> Result<()> {
  let now = Utc::now();
  let one_day_ago = (now - chrono::Duration::days(1)).to_rfc3339();
  let seven_days_ago = (now - chrono::Duration::days(7)).to_rfc3339();

  // Reset 7d and 1d counts, then recalculate from review_logs
  conn.execute("UPDATE character_stats SET attempts_7d = 0, correct_7d = 0, attempts_1d = 0, correct_1d = 0", [])?;

  // Recalculate 7d stats from review_logs joined with cards
  conn.execute(
    r#"
    UPDATE character_stats
    SET attempts_7d = (
      SELECT COUNT(*) FROM review_logs rl
      JOIN cards c ON rl.card_id = c.id
      WHERE (c.front = character_stats.character OR c.main_answer = character_stats.character)
        AND rl.reviewed_at >= ?1
    ),
    correct_7d = (
      SELECT COUNT(*) FROM review_logs rl
      JOIN cards c ON rl.card_id = c.id
      WHERE (c.front = character_stats.character OR c.main_answer = character_stats.character)
        AND rl.reviewed_at >= ?1
        AND rl.is_correct = 1
    )
    "#,
    params![seven_days_ago],
  )?;

  // Recalculate 1d stats
  conn.execute(
    r#"
    UPDATE character_stats
    SET attempts_1d = (
      SELECT COUNT(*) FROM review_logs rl
      JOIN cards c ON rl.card_id = c.id
      WHERE (c.front = character_stats.character OR c.main_answer = character_stats.character)
        AND rl.reviewed_at >= ?1
    ),
    correct_1d = (
      SELECT COUNT(*) FROM review_logs rl
      JOIN cards c ON rl.card_id = c.id
      WHERE (c.front = character_stats.character OR c.main_answer = character_stats.character)
        AND rl.reviewed_at >= ?1
        AND rl.is_correct = 1
    )
    "#,
    params![one_day_ago],
  )?;

  Ok(())
}

fn row_to_character_stats(row: &rusqlite::Row) -> Result<CharacterStats> {
  let last_attempt_str: Option<String> = row.get(8)?;
  Ok(CharacterStats {
    character: row.get(0)?,
    character_type: row.get(1)?,
    total_attempts: row.get(2)?,
    total_correct: row.get(3)?,
    attempts_7d: row.get(4)?,
    correct_7d: row.get(5)?,
    attempts_1d: row.get(6)?,
    correct_1d: row.get(7)?,
    last_attempt_at: last_attempt_str.and_then(|s| {
      DateTime::parse_from_rfc3339(&s)
        .map(|dt| dt.with_timezone(&Utc))
        .ok()
    }),
  })
}

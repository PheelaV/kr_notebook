use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result};

use crate::domain::{Card, CardType, ReviewLog};

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
           interval_days, repetitions, next_review, total_reviews, correct_reviews
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
  let now = Utc::now().to_rfc3339();
  let max_tier = get_max_unlocked_tier(conn)?;

  // If we have a last card, exclude its sibling (reverse card)
  if let Some(last_id) = exclude_sibling_of {
    // Get the last card's front and main_answer to identify siblings
    if let Ok(Some(last_card)) = get_card_by_id(conn, last_id) {
      let mut stmt = conn.prepare(
        r#"
        SELECT id, front, main_answer, description, card_type, tier, audio_hint, ease_factor,
               interval_days, repetitions, next_review, total_reviews, correct_reviews
        FROM cards
        WHERE next_review <= ?1 AND tier <= ?2
          AND id != ?3
          AND main_answer != ?4
          AND front NOT LIKE '%' || ?5 || '%'
        ORDER BY tier ASC, next_review ASC
        LIMIT ?6
        "#,
      )?;

      let cards = stmt
        .query_map(
          params![now, max_tier, last_id, last_card.front, last_card.main_answer, limit as i64],
          |row| row_to_card(row),
        )?
        .collect::<Result<Vec<_>>>()?;
      return Ok(cards);
    }
  }

  // Default: no sibling exclusion
  let mut stmt = conn.prepare(
    r#"
    SELECT id, front, main_answer, description, card_type, tier, audio_hint, ease_factor,
           interval_days, repetitions, next_review, total_reviews, correct_reviews
    FROM cards
    WHERE next_review <= ?1 AND tier <= ?2
    ORDER BY tier ASC, next_review ASC
    LIMIT ?3
    "#,
  )?;

  let cards = stmt
    .query_map(params![now, max_tier, limit as i64], |row| row_to_card(row))?
    .collect::<Result<Vec<_>>>()?;
  Ok(cards)
}

pub fn get_due_count(conn: &Connection) -> Result<i64> {
  let now = Utc::now().to_rfc3339();
  let max_tier = get_max_unlocked_tier(conn)?;
  conn.query_row(
    "SELECT COUNT(*) FROM cards WHERE next_review <= ?1 AND tier <= ?2",
    params![now, max_tier],
    |row| row.get(0),
  )
}

/// Get the next scheduled review time (for cards not yet due)
pub fn get_next_review_time(conn: &Connection) -> Result<Option<DateTime<Utc>>> {
  let now = Utc::now().to_rfc3339();
  let max_tier = get_max_unlocked_tier(conn)?;

  let result: std::result::Result<String, _> = conn.query_row(
    "SELECT next_review FROM cards WHERE next_review > ?1 AND tier <= ?2 ORDER BY next_review ASC LIMIT 1",
    params![now, max_tier],
    |row| row.get(0),
  );

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

/// Get cards for practice mode - any unlocked card, ordered by least recently reviewed
pub fn get_practice_cards(conn: &Connection, limit: usize, exclude_id: Option<i64>) -> Result<Vec<Card>> {
  let max_tier = get_max_unlocked_tier(conn)?;

  if let Some(last_id) = exclude_id {
    if let Ok(Some(last_card)) = get_card_by_id(conn, last_id) {
      let mut stmt = conn.prepare(
        r#"
        SELECT id, front, main_answer, description, card_type, tier, audio_hint, ease_factor,
               interval_days, repetitions, next_review, total_reviews, correct_reviews
        FROM cards
        WHERE tier <= ?1
          AND id != ?2
          AND main_answer != ?3
          AND front NOT LIKE '%' || ?4 || '%'
        ORDER BY RANDOM()
        LIMIT ?5
        "#,
      )?;

      let cards = stmt
        .query_map(
          params![max_tier, last_id, last_card.front, last_card.main_answer, limit as i64],
          |row| row_to_card(row),
        )?
        .collect::<Result<Vec<_>>>()?;
      return Ok(cards);
    }
  }

  let mut stmt = conn.prepare(
    r#"
    SELECT id, front, main_answer, description, card_type, tier, audio_hint, ease_factor,
           interval_days, repetitions, next_review, total_reviews, correct_reviews
    FROM cards
    WHERE tier <= ?1
    ORDER BY RANDOM()
    LIMIT ?2
    "#,
  )?;

  let cards = stmt
    .query_map(params![max_tier, limit as i64], |row| row_to_card(row))?
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
  correct: bool,
) -> Result<()> {
  conn.execute(
    r#"
    UPDATE cards
    SET ease_factor = ?1, interval_days = ?2, repetitions = ?3, next_review = ?4,
        total_reviews = total_reviews + 1,
        correct_reviews = correct_reviews + ?5
    WHERE id = ?6
    "#,
    params![
      ease_factor,
      interval_days,
      repetitions,
      next_review.to_rfc3339(),
      if correct { 1 } else { 0 },
      id,
    ],
  )?;
  Ok(())
}

pub fn insert_review_log(conn: &Connection, log: &ReviewLog) -> Result<i64> {
  conn.execute(
    "INSERT INTO review_logs (card_id, quality, reviewed_at) VALUES (?1, ?2, ?3)",
    params![log.card_id, log.quality, log.reviewed_at.to_rfc3339()],
  )?;
  Ok(conn.last_insert_rowid())
}

// Settings functions

pub fn get_max_unlocked_tier(conn: &Connection) -> Result<u8> {
  conn.query_row(
    "SELECT value FROM settings WHERE key = 'max_unlocked_tier'",
    [],
    |row| {
      let val: String = row.get(0)?;
      Ok(val.parse::<u8>().unwrap_or(1))
    },
  )
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

#[derive(Debug, Clone)]
pub struct TierProgress {
  pub tier: u8,
  pub total: i64,
  pub learned: i64,
  pub is_unlocked: bool,
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
  let mut stmt = conn.prepare(
    r#"
    SELECT tier,
           COUNT(*) as total,
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
        learned: row.get(2)?,
        is_unlocked: tier <= max_tier,
      })
    })?
    .collect::<Result<Vec<_>>>()?;
  Ok(progress)
}

pub fn get_total_stats(conn: &Connection) -> Result<(i64, i64, i64)> {
  let max_tier = get_max_unlocked_tier(conn)?;
  let total_cards: i64 = conn.query_row(
    "SELECT COUNT(*) FROM cards WHERE tier <= ?1",
    params![max_tier],
    |row| row.get(0),
  )?;
  let total_reviews: i64 = conn.query_row(
    "SELECT COALESCE(SUM(total_reviews), 0) FROM cards WHERE tier <= ?1",
    params![max_tier],
    |row| row.get(0),
  )?;
  let cards_learned: i64 = conn.query_row(
    "SELECT COUNT(*) FROM cards WHERE repetitions >= 2 AND tier <= ?1",
    params![max_tier],
    |row| row.get(0),
  )?;
  Ok((total_cards, total_reviews, cards_learned))
}

fn row_to_card(row: &rusqlite::Row) -> Result<Card> {
  let card_type_str: String = row.get(4)?;
  let next_review_str: String = row.get(10)?;

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
  })
}

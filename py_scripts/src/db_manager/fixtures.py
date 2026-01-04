"""Baseline card data and test fixtures for db_manager.

This module contains the same baseline cards as src/db/mod.rs::get_hangul_seed_data()
to ensure Python testing tools can create databases with identical seed data.
"""

from typing import TypedDict


class CardData(TypedDict, total=False):
    """Card data structure matching learning.db schema."""
    front: str
    main_answer: str
    description: str | None
    card_type: str
    tier: int
    is_reverse: bool


def _card(front: str, main: str, desc: str | None, card_type: str, tier: int) -> CardData:
    """Create a forward card (Korean -> romanization)."""
    return {
        "front": front,
        "main_answer": main,
        "description": desc,
        "card_type": card_type,
        "tier": tier,
        "is_reverse": False,
    }


def _reverse(romanization: str, korean: str, card_type: str, tier: int) -> CardData:
    """Create a reverse card (romanization -> Korean)."""
    return {
        "front": romanization,
        "main_answer": korean,
        "description": None,
        "card_type": card_type,
        "tier": tier,
        "is_reverse": True,
    }


# Card types (matching Rust CardType enum)
CONSONANT = "Consonant"
VOWEL = "Vowel"
ASPIRATED_CONSONANT = "AspiratedConsonant"
TENSE_CONSONANT = "TenseConsonant"
COMPOUND_VOWEL = "CompoundVowel"


def _build_baseline_cards() -> list[CardData]:
    """Build the baseline card list matching Rust get_hangul_seed_data()."""
    cards: list[CardData] = []

    # Tier 1: Basic Consonants (letter -> sound)
    tier1_consonants = [
        ("ㄱ", "g / k", "Like 'g' in 'go' at the start, 'k' in 'kite' at the end"),
        ("ㄴ", "n", "Like 'n' in 'no'"),
        ("ㄷ", "d / t", "Like 'd' in 'do' at the start, 't' in 'top' at the end"),
        ("ㄹ", "r / l", "Like 'r' in 'run' at the start, 'l' in 'ball' at the end"),
        ("ㅁ", "m", "Like 'm' in 'mom'"),
        ("ㅂ", "b / p", "Like 'b' in 'boy' at the start, 'p' in 'put' at the end"),
        ("ㅅ", "s", "Like 's' in 'sun'"),
        ("ㅈ", "j", "Like 'j' in 'just'"),
        ("ㅎ", "h", "Like 'h' in 'hi'"),
    ]

    for front, main, desc in tier1_consonants:
        cards.append(_card(front, main, desc, CONSONANT, 1))
        cards.append(_reverse(main, front, CONSONANT, 1))

    # Tier 1: Basic Vowels (letter -> sound)
    tier1_vowels = [
        ("ㅏ", "a", "Like 'a' in 'father'"),
        ("ㅓ", "eo", "Like 'u' in 'fun' or 'uh'"),
        ("ㅗ", "o", "Like 'o' in 'go'"),
        ("ㅜ", "u", "Like 'oo' in 'moon'"),
        ("ㅡ", "eu", "Like 'u' in 'put', with unrounded lips"),
        ("ㅣ", "i", "Like 'ee' in 'see'"),
    ]

    for front, main, desc in tier1_vowels:
        cards.append(_card(front, main, desc, VOWEL, 1))
        cards.append(_reverse(main, front, VOWEL, 1))

    # Tier 2: ㅇ and Y-vowels
    cards.append(_card(
        "ㅇ (initial)",
        "Silent",
        "No sound when at the start of a syllable",
        CONSONANT,
        2,
    ))
    cards.append(_card(
        "ㅇ (final)",
        "ng",
        "Like 'ng' in 'sing' when at the end",
        CONSONANT,
        2,
    ))

    tier2_vowels = [
        ("ㅑ", "ya", "Like 'ya' in 'yacht'"),
        ("ㅕ", "yeo", "Like 'yu' in 'yuck'"),
        ("ㅛ", "yo", "Like 'yo' in 'yoga'"),
        ("ㅠ", "yu", "Like 'you'"),
        ("ㅐ", "ae", "Like 'a' in 'can' or 'e' in 'bed'"),
        ("ㅔ", "e", "Like 'e' in 'bed' (sounds same as ㅐ in modern Korean)"),
    ]

    for front, main, desc in tier2_vowels:
        cards.append(_card(front, main, desc, VOWEL, 2))
        cards.append(_reverse(main, front, VOWEL, 2))

    # Tier 3: Aspirated Consonants
    tier3_aspirated = [
        ("ㅋ", "k (aspirated)", "Stronger 'k' with a puff of breath, like 'k' in 'kick'"),
        ("ㅍ", "p (aspirated)", "Stronger 'p' with a puff of breath, like 'p' in 'pop'"),
        ("ㅌ", "t (aspirated)", "Stronger 't' with a puff of breath, like 't' in 'top'"),
        ("ㅊ", "ch (aspirated)", "Stronger 'ch' with a puff of breath, like 'ch' in 'church'"),
    ]

    for front, main, desc in tier3_aspirated:
        cards.append(_card(front, main, desc, ASPIRATED_CONSONANT, 3))
        cards.append(_reverse(main, front, ASPIRATED_CONSONANT, 3))

    # Tier 3: Tense Consonants
    tier3_tense = [
        ("ㄲ", "kk (tense)", "Tense 'k' with no breath, like 'ck' in 'sticky'"),
        ("ㅃ", "pp (tense)", "Tense 'p' with no breath, like 'pp' in 'happy'"),
        ("ㄸ", "tt (tense)", "Tense 't' with no breath, like 'tt' in 'butter'"),
        ("ㅆ", "ss (tense)", "Tense 's', like 'ss' in 'hiss'"),
        ("ㅉ", "jj (tense)", "Tense 'j', like 'dg' in 'edge'"),
    ]

    for front, main, desc in tier3_tense:
        cards.append(_card(front, main, desc, TENSE_CONSONANT, 3))
        cards.append(_reverse(main, front, TENSE_CONSONANT, 3))

    # Tier 4: Compound Vowels
    tier4_compound = [
        ("ㅘ", "wa", "Like 'wa' in 'want'"),
        ("ㅝ", "wo", "Like 'wo' in 'won'"),
        ("ㅟ", "wi", "Like 'wee'"),
        ("ㅚ", "oe", "Like 'we' in 'wet'"),
        ("ㅢ", "ui", "Like 'oo-ee' said quickly"),
        ("ㅙ", "wae", "Like 'wa' in 'wax'"),
        ("ㅞ", "we", "Like 'we' in 'wet'"),
        ("ㅒ", "yae", "Like 'ya' in 'yam'"),
        ("ㅖ", "ye", "Like 'ye' in 'yes'"),
    ]

    for front, main, desc in tier4_compound:
        cards.append(_card(front, main, desc, COMPOUND_VOWEL, 4))
        cards.append(_reverse(main, front, COMPOUND_VOWEL, 4))

    return cards


# Exported baseline cards list
BASELINE_CARDS: list[CardData] = _build_baseline_cards()

# Card counts by tier for verification
CARD_COUNTS = {
    1: 30,  # 9 consonants + 6 vowels, each with reverse = 30
    2: 14,  # 2 ㅇ cards (no reverse) + 6 vowels with reverse = 14
    3: 18,  # 4 aspirated + 5 tense, each with reverse = 18
    4: 18,  # 9 compound vowels, each with reverse = 18
    "total": 80,
}


def verify_baseline_cards() -> bool:
    """Verify baseline cards match expected counts."""
    by_tier: dict[int, int] = {}
    for card in BASELINE_CARDS:
        tier = card["tier"]
        by_tier[tier] = by_tier.get(tier, 0) + 1

    expected_total = sum(CARD_COUNTS[t] for t in [1, 2, 3, 4])
    actual_total = len(BASELINE_CARDS)

    if actual_total != expected_total:
        print(f"Total mismatch: expected {expected_total}, got {actual_total}")
        return False

    for tier in [1, 2, 3, 4]:
        if by_tier.get(tier, 0) != CARD_COUNTS[tier]:
            print(f"Tier {tier} mismatch: expected {CARD_COUNTS[tier]}, got {by_tier.get(tier, 0)}")
            return False

    return True


if __name__ == "__main__":
    # Quick verification when run directly
    print(f"Total baseline cards: {len(BASELINE_CARDS)}")
    by_tier: dict[int, int] = {}
    for card in BASELINE_CARDS:
        tier = card["tier"]
        by_tier[tier] = by_tier.get(tier, 0) + 1
    for tier in sorted(by_tier.keys()):
        print(f"  Tier {tier}: {by_tier[tier]} cards")

    if verify_baseline_cards():
        print("Verification passed!")
    else:
        print("Verification FAILED!")

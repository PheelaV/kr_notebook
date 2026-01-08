"""Command-line interface for Korean content scraper."""

import json
import shutil
from pathlib import Path

import click

from .paths import HTSK_DIR, PROJECT_ROOT

# Default output directory for scraped content
DEFAULT_OUTPUT = HTSK_DIR


@click.group()
@click.version_option()
def cli() -> None:
    """Korean content scraper for howtostudykorean.com.

    Scrape pronunciation audio files for Korean learning.
    Similar to youtube-dl: provides tools to fetch content
    that isn't distributed with the app.
    """
    pass


@cli.command()
@click.option(
    "--output",
    "-o",
    type=click.Path(path_type=Path),
    default=None,
    help="Output directory for audio files.",
)
@click.option(
    "--force",
    "-f",
    is_flag=True,
    help="Re-download files even if they exist.",
)
def lesson1(output: Path | None, force: bool) -> None:
    """Scrape Lesson 1 pronunciation audio.

    Downloads vowel and consonant pronunciation MP3 files
    from howtostudykorean.com Unit 0 Lesson 1.

    Includes 9 basic consonants (ㅂㅈㄷㄱㅅㅁㄴㅎㄹ) and 6 vowels (ㅣㅏㅓㅡㅜㅗ).
    """
    from .lesson1 import scrape_lesson1

    output_dir = output or (DEFAULT_OUTPUT / "lesson1")

    click.echo(f"Scraping Lesson 1 pronunciation audio...")
    click.echo(f"Output: {output_dir}")
    click.echo()

    def progress(current: int, total: int, char: str, success: bool) -> None:
        status = click.style("OK", fg="green") if success else click.style("FAIL", fg="red")
        click.echo(f"  [{current}/{total}] {char} ... {status}")

    try:
        manifest = scrape_lesson1(
            output_dir=output_dir,
            progress_callback=progress,
            skip_existing=not force,
        )

        columns = len(manifest.get("columns", {}))
        rows = len(manifest.get("rows", {}))
        click.echo()
        click.echo(f"Downloaded {columns} column + {rows} row audio files.")
        click.echo(f"Manifest saved to {output_dir / 'manifest.json'}")

    except Exception as e:
        raise click.ClickException(str(e)) from e


@cli.command()
@click.option(
    "--output",
    "-o",
    type=click.Path(path_type=Path),
    default=None,
    help="Output directory for audio files.",
)
@click.option(
    "--force",
    "-f",
    is_flag=True,
    help="Re-download files even if they exist.",
)
def lesson2(output: Path | None, force: bool) -> None:
    """Scrape Lesson 2 consonant pronunciation audio.

    Downloads audio for additional consonants from Unit 0 Lesson 2:
    - Silent/ng: ㅇ
    - Double (tense): ㄲ ㅃ ㅉ ㄸ ㅆ
    - Aspirated: ㅋ ㅍ ㅊ ㅌ

    Note: These are individual consonant sounds, not row/column audio.
    Segmentation is not applicable for Lesson 2.
    """
    from .lesson2 import scrape_lesson2

    output_dir = output or (DEFAULT_OUTPUT / "lesson2")

    click.echo(f"Scraping Lesson 2 consonant audio...")
    click.echo(f"Output: {output_dir}")
    click.echo()

    def progress(current: int, total: int, char: str, success: bool) -> None:
        status = click.style("OK", fg="green") if success else click.style("FAIL", fg="red")
        click.echo(f"  [{current}/{total}] {char} ... {status}")

    try:
        manifest = scrape_lesson2(
            output_dir=output_dir,
            progress_callback=progress,
            skip_existing=not force,
        )

        rows = len(manifest.get("rows", {}))
        click.echo()
        click.echo(f"Downloaded {rows} row audio files.")
        click.echo(f"Manifest saved to {output_dir / 'manifest.json'}")

    except Exception as e:
        raise click.ClickException(str(e)) from e


@cli.command()
@click.option(
    "--path",
    "-p",
    type=click.Path(path_type=Path),
    default=None,
    help="Path to check (default: standard scraped data location).",
)
def status(path: Path | None) -> None:
    """Show status of scraped content.

    Lists what audio files have been scraped and their locations.
    """
    check_path = path or DEFAULT_OUTPUT

    if not check_path.exists():
        click.echo("No scraped content found.")
        click.echo(f"Expected location: {check_path}")
        click.echo()
        click.echo("Run 'kr-scraper lesson1' and 'kr-scraper lesson2' to scrape pronunciation audio.")
        return

    click.echo(f"Scraped content in: {check_path}")
    click.echo()

    # Check for lesson1
    lesson1_dir = check_path / "lesson1"
    if lesson1_dir.exists():
        manifest_path = lesson1_dir / "manifest.json"
        if manifest_path.exists():
            with open(manifest_path, encoding="utf-8") as f:
                manifest = json.load(f)

            scraped_at = manifest.get("scraped_at", "unknown")
            columns = manifest.get("columns", {})
            rows = manifest.get("rows", {})
            syllable_table = manifest.get("syllable_table", {})

            click.echo(click.style("Lesson 1 (Basic Consonants & Vowels):", bold=True))
            click.echo(f"  Scraped: {scraped_at}")

            # Column audio (vowels)
            click.echo(f"  Column audio (vowels): {len(columns)} files")
            if columns:
                vowels = " ".join(columns.keys())
                click.echo(f"    {vowels}")

            # Row audio (consonants)
            click.echo(f"  Row audio (consonants): {len(rows)} files")
            if rows:
                consonants = " ".join(rows.keys())
                click.echo(f"    {consonants}")

            # Syllable segments
            syllables_dir = lesson1_dir / "syllables"
            segment_count = len(list(syllables_dir.glob("*.mp3"))) if syllables_dir.exists() else 0
            total_syllables = len(syllable_table)
            click.echo(f"  Individual syllables: {segment_count}/{total_syllables} segmented")
            if segment_count == 0:
                click.echo("    Run 'kr-scraper segment' to extract individual syllables")
            click.echo()

        else:
            col_count = len(list((lesson1_dir / "columns").glob("*.mp3"))) if (lesson1_dir / "columns").exists() else 0
            row_count = len(list((lesson1_dir / "rows").glob("*.mp3"))) if (lesson1_dir / "rows").exists() else 0
            click.echo(f"Lesson 1: {col_count} column + {row_count} row files (no manifest)")
            click.echo()
    else:
        click.echo(click.style("Lesson 1:", bold=True) + " Not scraped")
        click.echo("  Run 'kr-scraper lesson1' to download")
        click.echo()

    # Check for lesson2
    lesson2_dir = check_path / "lesson2"
    if lesson2_dir.exists():
        manifest_path = lesson2_dir / "manifest.json"
        if manifest_path.exists():
            with open(manifest_path, encoding="utf-8") as f:
                manifest = json.load(f)

            scraped_at = manifest.get("scraped_at", "unknown")
            rows = manifest.get("rows", {})
            syllable_table = manifest.get("syllable_table", {})

            click.echo(click.style("Lesson 2 (Additional Consonants):", bold=True))
            click.echo(f"  Scraped: {scraped_at}")

            # Row audio (consonants)
            click.echo(f"  Row audio (consonants): {len(rows)} files")
            if rows:
                chars = " ".join(rows.keys())
                click.echo(f"    {chars}")

            # Syllable segments
            syllables_dir = lesson2_dir / "syllables"
            segment_count = len(list(syllables_dir.glob("*.mp3"))) if syllables_dir.exists() else 0
            total_syllables = len(syllable_table)
            click.echo(f"  Individual syllables: {segment_count}/{total_syllables} segmented")
            if segment_count == 0 and len(rows) > 0:
                click.echo("    Run 'kr-scraper segment -l 2' to extract individual syllables")
            click.echo()

        else:
            row_count = len(list((lesson2_dir / "rows").glob("*.mp3"))) if (lesson2_dir / "rows").exists() else 0
            click.echo(f"Lesson 2: {row_count} row files (no manifest)")
            click.echo()
    else:
        click.echo(click.style("Lesson 2:", bold=True) + " Not scraped")
        click.echo("  Run 'kr-scraper lesson2' to download")
        click.echo()

    # Check for lesson3
    lesson3_dir = check_path / "lesson3"
    if lesson3_dir.exists():
        manifest_path = lesson3_dir / "manifest.json"
        if manifest_path.exists():
            with open(manifest_path, encoding="utf-8") as f:
                manifest = json.load(f)

            scraped_at = manifest.get("scraped_at", "unknown")
            rows = manifest.get("rows", {})
            syllable_table = manifest.get("syllable_table", {})
            vowels_order = manifest.get("vowels_order", [])

            click.echo(click.style("Lesson 3 (Diphthongs & Combined Vowels):", bold=True))
            click.echo(f"  Scraped: {scraped_at}")

            # Row audio (vowels)
            click.echo(f"  Row audio (vowels): {len(rows)} files")
            if rows:
                vowels = " ".join(vowels_order)
                click.echo(f"    {vowels}")

            # Syllable segments
            syllables_dir = lesson3_dir / "syllables"
            segment_count = len(list(syllables_dir.glob("*.mp3"))) if syllables_dir.exists() else 0
            total_syllables = len(syllable_table)
            click.echo(f"  Individual syllables: {segment_count}/{total_syllables} segmented")
            if segment_count == 0 and len(rows) > 0:
                click.echo("    Run 'kr-scraper segment -l 3' to extract individual syllables")
            click.echo()

        else:
            row_count = len(list((lesson3_dir / "rows").glob("*.mp3"))) if (lesson3_dir / "rows").exists() else 0
            click.echo(f"Lesson 3: {row_count} row files (no manifest)")
            click.echo()
    else:
        click.echo(click.style("Lesson 3:", bold=True) + " Not scraped")
        click.echo("  Run 'kr-scraper lesson3' to download")
        click.echo()


@cli.command()
@click.option(
    "--output",
    "-o",
    type=click.Path(path_type=Path),
    default=None,
    help="Output directory for audio files.",
)
@click.option(
    "--force",
    "-f",
    is_flag=True,
    help="Re-download files even if they exist.",
)
def lesson3(output: Path | None, force: bool) -> None:
    """Scrape Lesson 3 diphthong and combined vowel audio.

    Downloads audio for new vowels from Unit 0 Lesson 3:
    - Combined vowels: ㅐ ㅔ ㅒ ㅖ
    - Diphthongs: ㅘ ㅙ ㅚ ㅝ ㅞ ㅟ ㅢ

    Each audio file demonstrates a vowel with various consonants.
    """
    from .lesson3 import scrape_lesson3

    output_dir = output or (DEFAULT_OUTPUT / "lesson3")

    click.echo("Scraping Lesson 3 vowel audio...")
    click.echo(f"Output: {output_dir}")
    click.echo()

    def progress(current: int, total: int, vowel: str, success: bool) -> None:
        status = click.style("OK", fg="green") if success else click.style("FAIL", fg="red")
        click.echo(f"  [{current}/{total}] {vowel} ... {status}")

    try:
        manifest = scrape_lesson3(
            output_dir=output_dir,
            progress_callback=progress,
            skip_existing=not force,
        )

        rows = len(manifest.get("rows", {}))
        syllables = len(manifest.get("syllable_table", {}))
        click.echo()
        click.echo(f"Downloaded {rows} vowel row audio files ({syllables} syllables).")
        click.echo(f"Manifest saved to {output_dir / 'manifest.json'}")

    except Exception as e:
        raise click.ClickException(str(e)) from e


@cli.command()
@click.option(
    "--lesson",
    "-l",
    type=click.Choice(["1", "2", "3", "all"]),
    default="all",
    help="Which lesson to segment (1, 2, 3, or all).",
)
@click.option(
    "--path",
    "-p",
    type=click.Path(path_type=Path),
    default=None,
    help="Lesson directory to segment (overrides --lesson).",
)
@click.option(
    "--min-silence",
    "-s",
    type=int,
    default=200,
    help="Minimum silence duration (ms) to split on.",
)
@click.option(
    "--threshold",
    "-t",
    type=int,
    default=-40,
    help="Silence threshold in dBFS (lower = more sensitive).",
)
@click.option(
    "--padding",
    "-P",
    type=int,
    default=50,
    help="Padding (ms) to add before/after each segment to avoid abrupt cuts.",
)
@click.option(
    "--reset",
    "-r",
    is_flag=True,
    default=False,
    help="Reset/ignore saved manifest params and use CLI values for all rows.",
)
def segment(lesson: str, path: Path | None, min_silence: int, threshold: int, padding: int, reset: bool) -> None:
    """Segment row/column audio into individual syllables.

    Uses silence detection to extract each syllable from the
    row/column audio files. Requires ffmpeg to be installed.
    """
    from .segment import SegmentResult, segment_lesson1

    # Determine which lessons to process
    if path:
        lesson_dirs = [(path, "custom")]
    elif lesson == "all":
        lesson_dirs = []
        if (DEFAULT_OUTPUT / "lesson1").exists():
            lesson_dirs.append((DEFAULT_OUTPUT / "lesson1", "lesson1"))
        if (DEFAULT_OUTPUT / "lesson2").exists():
            lesson_dirs.append((DEFAULT_OUTPUT / "lesson2", "lesson2"))
        if (DEFAULT_OUTPUT / "lesson3").exists():
            lesson_dirs.append((DEFAULT_OUTPUT / "lesson3", "lesson3"))
        if not lesson_dirs:
            raise click.ClickException(
                "No lesson directories found.\n"
                "Run 'kr-scraper lesson1', 'kr-scraper lesson2', or 'kr-scraper lesson3' first."
            )
    else:
        lesson_dir = DEFAULT_OUTPUT / f"lesson{lesson}"
        if not lesson_dir.exists():
            raise click.ClickException(
                f"Lesson {lesson} directory not found: {lesson_dir}\n"
                f"Run 'kr-scraper lesson{lesson}' first to download audio."
            )
        lesson_dirs = [(lesson_dir, f"lesson{lesson}")]

    click.echo(f"Settings: min_silence={min_silence}ms, threshold={threshold}dBFS, padding={padding}ms" + (" (reset)" if reset else ""))
    click.echo()

    all_mismatches: list[SegmentResult] = []

    for lesson_dir, lesson_name in lesson_dirs:
        click.echo(click.style(f"=== {lesson_name.upper()} ===", bold=True))
        click.echo(f"Segmenting audio from: {lesson_dir}")
        click.echo()

        mismatches: list[SegmentResult] = []

        def progress(result: SegmentResult) -> None:
            expected = len(result.syllables)
            found = result.segments_found
            saved = result.segments_saved

            # Determine status color
            if result.mismatch and not result.override_applied:
                status = click.style("MISMATCH", fg="red")
                mismatches.append(result)
            elif result.override_applied:
                status = click.style(f"OK (override: {result.override_applied})", fg="cyan")
            elif saved == expected:
                status = click.style("OK", fg="green")
            else:
                status = click.style("?", fg="yellow")

            click.echo(f"  {result.source_label}: {found} found, {expected} expected, {saved} saved ... {status}")

        try:
            results = segment_lesson1(
                lesson_dir=lesson_dir,
                progress_callback=progress,
                min_silence_len=min_silence,
                silence_thresh=threshold,
                padding_ms=padding,
                reset_params=reset,
            )

            total_found = sum(r.segments_found for r in results.values())
            total_saved = sum(r.segments_saved for r in results.values())
            total_expected = sum(len(r.syllables) for r in results.values())

            click.echo()
            click.echo(f"  Complete: {total_saved}/{total_expected} syllables extracted from {len(results)} files.")
            click.echo(f"  Saved to: {lesson_dir / 'syllables'}")
            click.echo()

            all_mismatches.extend(mismatches)

        except FileNotFoundError as e:
            click.echo(click.style(f"  Error: {e}", fg="red"))
            click.echo()
        except Exception as e:
            click.echo(click.style(f"  Error: {e}", fg="red"))
            click.echo()

    # Report detailed mismatches
    if all_mismatches:
        click.echo(click.style("Mismatches detected:", fg="yellow", bold=True))
        for result in all_mismatches:
            expected = len(result.syllables)
            found = result.segments_found
            diff = found - expected

            click.echo(f"\n  {result.source_label}:")
            click.echo(f"    File: {result.source_file.name}")
            click.echo(f"    Expected {expected} syllables: {' '.join(result.syllables)}")
            click.echo(f"    Found {found} segments ({'+' if diff > 0 else ''}{diff})")

            if diff > 0:
                click.echo(f"    Suggestion: Add to AUDIO_OVERRIDES in segment.py:")
                click.echo(click.style(f'      "{result.source_file.name}": {{"skip_first": {diff}}},', fg="cyan"))


@cli.command()
@click.option(
    "--path",
    "-p",
    type=click.Path(path_type=Path),
    default=None,
    help="Path to clean (default: standard scraped data location).",
)
@click.option(
    "--yes",
    "-y",
    is_flag=True,
    help="Skip confirmation prompt.",
)
def clean(path: Path | None, yes: bool) -> None:
    """Remove all scraped content.

    Deletes all downloaded audio files and manifests.
    """
    clean_path = path or DEFAULT_OUTPUT

    if not clean_path.exists():
        click.echo("No scraped content to clean.")
        return

    # Count what will be deleted
    mp3_count = len(list(clean_path.rglob("*.mp3")))
    manifest_count = len(list(clean_path.rglob("manifest.json")))

    if mp3_count == 0 and manifest_count == 0:
        click.echo("No scraped content to clean.")
        return

    click.echo(f"Will delete from: {clean_path}")
    click.echo(f"  - {mp3_count} audio files")
    click.echo(f"  - {manifest_count} manifest files")

    if not yes:
        if not click.confirm("Continue?"):
            click.echo("Aborted.")
            return

    shutil.rmtree(clean_path)
    click.echo("Scraped content removed.")


@cli.command("segment-row")
@click.argument("lesson")
@click.argument("row")
@click.option(
    "--min-silence",
    "-s",
    type=int,
    default=200,
    help="Minimum silence duration (ms) to split on.",
)
@click.option(
    "--threshold",
    "-t",
    type=int,
    default=-40,
    help="Silence threshold in dBFS.",
)
@click.option(
    "--padding",
    "-P",
    type=int,
    default=50,
    help="Padding (ms) before/after each segment.",
)
@click.option(
    "--skip-first",
    type=int,
    default=0,
    help="Skip first N detected segments.",
)
@click.option(
    "--skip-last",
    type=int,
    default=0,
    help="Skip last N detected segments.",
)
@click.option(
    "--json",
    "output_json",
    is_flag=True,
    help="Output result as JSON.",
)
def segment_row(
    lesson: str,
    row: str,
    min_silence: int,
    threshold: int,
    padding: int,
    skip_first: int,
    skip_last: int,
    output_json: bool,
) -> None:
    """Segment a single row with custom parameters.

    LESSON is the lesson identifier (e.g., 'lesson1' or 'lesson2').
    ROW is the row romanization (e.g., 'b', 'd', 'g').

    This command is used by the web UI for per-row parameter tuning.
    """
    from .segment import segment_single_row

    lesson_dir = DEFAULT_OUTPUT / lesson
    if not lesson_dir.exists():
        if output_json:
            print(json.dumps({"error": f"Lesson directory not found: {lesson_dir}"}))
        else:
            raise click.ClickException(f"Lesson directory not found: {lesson_dir}")
        return

    result = segment_single_row(
        lesson_dir=lesson_dir,
        row_romanization=row,
        min_silence=min_silence,
        threshold=threshold,
        padding=padding,
        skip_first=skip_first,
        skip_last=skip_last,
    )

    if output_json:
        print(
            json.dumps(
                {
                    "saved": result.segments_saved if result else 0,
                    "found": result.segments_found if result else 0,
                }
            )
        )
    elif result:
        click.echo(f"Segmented {row}: {result.segments_saved}/{result.segments_found} saved")
    else:
        click.echo(f"Row {row} not found in lesson {lesson}")


@cli.command("apply-manual")
@click.argument("lesson")
@click.argument("syllable")
@click.option(
    "--start",
    type=int,
    required=True,
    help="Start time in milliseconds (relative to source row audio).",
)
@click.option(
    "--end",
    type=int,
    required=True,
    help="End time in milliseconds.",
)
@click.option(
    "--padding",
    "-P",
    type=int,
    default=75,
    help="Padding (ms) before/after the segment.",
)
def apply_manual(lesson: str, syllable: str, start: int, end: int, padding: int) -> None:
    """Apply manual segment timestamps for a syllable.

    LESSON is the lesson identifier (e.g., 'lesson1', 'lesson3').
    SYLLABLE is the Korean character (e.g., '가', '애').

    Re-extracts the syllable audio using the specified timestamps
    and stores the manual override in the manifest.
    """
    from .segment import apply_manual_segment

    lesson_dir = DEFAULT_OUTPUT / lesson
    if not lesson_dir.exists():
        raise click.ClickException(f"Lesson directory not found: {lesson_dir}")

    success = apply_manual_segment(
        lesson_dir=lesson_dir,
        syllable=syllable,
        start_ms=start,
        end_ms=end,
        padding_ms=padding,
    )

    if success:
        click.echo(f"Applied manual segment for {syllable}: {start}-{end}ms (padding={padding}ms)")
    else:
        raise click.ClickException(f"Syllable '{syllable}' not found in lesson {lesson}")


@cli.command("reset-manual")
@click.argument("lesson")
@click.argument("syllable")
def reset_manual(lesson: str, syllable: str) -> None:
    """Reset manual segment timestamps to baseline.

    LESSON is the lesson identifier (e.g., 'lesson1', 'lesson3').
    SYLLABLE is the Korean character (e.g., '가', '애').

    Re-extracts the syllable audio using the baseline timestamps
    and removes the manual override from the manifest.
    """
    from .segment import reset_manual_segment

    lesson_dir = DEFAULT_OUTPUT / lesson
    if not lesson_dir.exists():
        raise click.ClickException(f"Lesson directory not found: {lesson_dir}")

    success = reset_manual_segment(
        lesson_dir=lesson_dir,
        syllable=syllable,
    )

    if success:
        click.echo(f"Reset {syllable} to baseline timestamps")
    else:
        raise click.ClickException(f"Syllable '{syllable}' not found or no baseline available")


@cli.command()
@click.argument(
    "input_path",
    type=click.Path(path_type=Path, exists=True),
)
@click.option(
    "--output",
    "-o",
    "output_path",
    type=click.Path(path_type=Path),
    default=None,
    help="Output cards.json path (default: same directory as input).",
)
@click.option(
    "--tier",
    "-t",
    type=int,
    default=5,
    help="Card tier (default: 5).",
)
@click.option(
    "--no-reverse",
    is_flag=True,
    help="Don't create reverse cards.",
)
def vocabulary(
    input_path: Path,
    output_path: Path | None,
    tier: int,
    no_reverse: bool,
) -> None:
    """Convert vocabulary.json to cards.json format.

    INPUT_PATH is a vocabulary.json file with entries containing:
    term, romanization, translation, word_type.

    Creates flashcards in the format expected by kr_notebook packs.
    By default, creates both forward (Korean -> English) and reverse
    (English -> Korean) cards.

    Example:
        kr-scraper vocabulary path/to/vocabulary.json
    """
    from .vocabulary import convert_vocabulary

    vocab_path = input_path
    cards_path = output_path or vocab_path.parent / "cards.json"

    click.echo("Converting vocabulary to cards...")
    click.echo(f"  Input: {vocab_path}")
    click.echo(f"  Output: {cards_path}")
    click.echo(f"  Tier: {tier}")
    click.echo(f"  Reverse cards: {not no_reverse}")
    click.echo()

    try:
        result = convert_vocabulary(
            vocab_path=vocab_path,
            output_path=cards_path,
            tier=tier,
            create_reverse=not no_reverse,
        )

        click.echo(f"Converted {result['vocabulary_count']} vocabulary entries")
        click.echo(f"Created {result['cards_created']} cards")
        click.echo(f"Output: {result['output']}")

    except Exception as e:
        raise click.ClickException(str(e)) from e

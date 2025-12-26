"""Korean content scraper for howtostudykorean.com pronunciation audio."""

from .cli import cli

__all__ = ["cli", "main"]


def main() -> None:
    """Entry point for the kr-scraper CLI."""
    cli()

"""Database management CLI for kr_notebook testing scenarios."""

from .cli import cli

__all__ = ["cli", "main"]


def main() -> None:
    """Entry point for the db-manager CLI."""
    cli()

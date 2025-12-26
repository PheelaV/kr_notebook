"""Utility functions for HTTP fetching and file I/O."""

import time
from pathlib import Path

import requests
from bs4 import BeautifulSoup

# Respect the site - reasonable delay between requests
REQUEST_DELAY_SECONDS = 0.5
USER_AGENT = "kr-scraper/1.0 (Korean learning app; educational use)"


def fetch_page(url: str) -> str:
    """Fetch HTML content from a URL.

    Args:
        url: The URL to fetch.

    Returns:
        The HTML content as a string.

    Raises:
        requests.RequestException: If the request fails.
    """
    headers = {"User-Agent": USER_AGENT}
    response = requests.get(url, headers=headers, timeout=30)
    response.raise_for_status()
    return response.text


def parse_html(html: str) -> BeautifulSoup:
    """Parse HTML content into a BeautifulSoup object.

    Args:
        html: Raw HTML string.

    Returns:
        Parsed BeautifulSoup object.
    """
    return BeautifulSoup(html, "html.parser")


def download_file(url: str, output_path: Path) -> bool:
    """Download a file from a URL.

    Args:
        url: The URL to download from.
        output_path: Where to save the file.

    Returns:
        True if downloaded successfully, False otherwise.
    """
    try:
        headers = {"User-Agent": USER_AGENT}
        response = requests.get(url, headers=headers, timeout=60, stream=True)
        response.raise_for_status()

        output_path.parent.mkdir(parents=True, exist_ok=True)
        with open(output_path, "wb") as f:
            for chunk in response.iter_content(chunk_size=8192):
                f.write(chunk)

        # Be polite - delay between downloads
        time.sleep(REQUEST_DELAY_SECONDS)
        return True

    except requests.RequestException:
        return False


def ensure_directory(path: Path) -> None:
    """Ensure a directory exists, creating it if necessary."""
    path.mkdir(parents=True, exist_ok=True)

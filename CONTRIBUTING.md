# Contributing to kr_notebook

Thank you for your interest in contributing!

## Prerequisites

See [README.md → Development → Prerequisites](README.md#prerequisites) for required tools:
- Rust 1.80+, Node.js 18+, Python 3.12+, uv, Tailwind CSS v4

Quick test dependency setup:
```bash
cd tests/e2e && npm install && npx playwright install && cd ../..
cd tests/js && npm install && cd ../..
cd tests/integration && uv sync && cd ../..
```

## How to Contribute

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/my-feature`)
3. Make your changes
4. Run tests (`./scripts/test.sh unit` for fast feedback, `./scripts/test.sh all` before PR)
5. Commit your changes with a descriptive message
6. Push to your fork and open a Pull Request

## Code Style

- Run `cargo clippy` before submitting
- Run `cargo fmt` for consistent formatting
- Follow existing code patterns in the codebase
- Add tests for new functionality

## Contributor License Agreement

By submitting a pull request, you agree that:

1. You have the right to submit the contribution
2. Your contribution is licensed under AGPL-3.0 (the project's current license)
3. You grant the project maintainers the right to relicense your contribution
   under the MIT license in the future, should the project change licenses

This clause exists solely to preserve the option of moving to a more permissive
license (MIT) in the future. Your contribution will never be relicensed under
a more restrictive license.

## Questions?

Open an issue if you have questions or need help getting started.

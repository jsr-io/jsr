# Contributing to JSR

Thank you for your interest in contributing to JSR! This document provides guidelines for contributing to the project.

## Getting Started

Please refer to the [README.md](README.md) for detailed setup instructions.

### Quick Start (Frontend Only)

If you're only making frontend changes:

1. Install [Deno](https://deno.land/#installation)
2. Add the required entries to your `/etc/hosts` file
3. Run `deno task prod:frontend`

### Full Stack Development

For API changes, you'll additionally need:

1. [Rust](https://rustup.rs/)
2. PostgreSQL
3. See the README for complete setup instructions

## How to Contribute

### Reporting Issues

Before creating an issue:

1. Search existing issues to avoid duplicates
2. If the issue is a security vulnerability, please email security@deno.com instead

When creating an issue, include:

- Clear title and description
- Steps to reproduce (for bugs)
- Expected vs actual behavior
- Your environment details

### Pull Requests

1. Fork the repository
2. Create a feature branch from `main`
3. Make your changes
4. Write or update tests as needed
5. Ensure all tests pass
6. Submit a pull request

#### Commit Messages

- Use clear, descriptive commit messages
- Reference related issues when applicable

#### Code Style

- **Frontend (TypeScript/TSX)**: Follow Deno formatting conventions
- **Backend (Rust)**: Follow Rust formatting conventions (`cargo fmt`)

### Areas for Contribution

- **Frontend**: UI improvements, accessibility, documentation pages
- **API**: New endpoints, bug fixes, performance improvements
- **Documentation**: Improve guides, fix typos, add examples
- **Tests**: Increase test coverage

## Documentation Contributions

Documentation lives in `frontend/docs/`. These are Markdown files that are rendered on jsr.io/docs.

For documentation generation (deno_doc) changes, please contribute to [deno_doc](https://github.com/denoland/deno_doc) directly.

## Code of Conduct

Please be respectful and constructive in all interactions. We're all here to make JSR better!

## Questions?

- For general questions, open a GitHub Discussion
- For security issues, email security@deno.com
- For registry support, email help@jsr.io

## License

By contributing to JSR, you agree that your contributions will be licensed under the MIT License.

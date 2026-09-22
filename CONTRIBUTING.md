# Contributing to Mint

Thanks for helping improve Mint. Contributions can include code, documentation, examples, tests, issue reports, and feedback from real-world use.

## Before you start

- Search existing [Issues](https://github.com/Pheem49/Mint/issues) and [Discussions](https://github.com/Pheem49/Mint/discussions) first.
- Use a Discussion for questions, ideas, and design proposals.
- Use an Issue for a reproducible bug or a narrowly scoped task.
- For security-sensitive reports, contact the maintainer privately rather than opening a public issue.

## Local setup

Mint uses Rust for the agent and CLI, and Node.js for the desktop and web surfaces.

```bash
git clone https://github.com/Pheem49/Mint.git
cd Mint
npm install
npm run typecheck
cargo test -p mint-core -p mint-cli
```

Create a branch for your change:

```bash
git checkout -b codex/short-description
```

## Making a change

Keep pull requests focused. Explain the user problem, the approach, and how you tested it. Update documentation when behavior or setup changes. If a change affects the release experience, update `Release_Note.md` as well.

Before opening a pull request, run the checks that match your change:

```bash
npm run typecheck
cargo fmt --all -- --check
cargo test -p mint-core -p mint-cli
```

## Good first issues

Issues marked `good first issue` should have a small scope, a clear definition of done, and enough context to work without private maintainer knowledge. If you are new to the project, start with the [good first issue list](https://github.com/Pheem49/Mint/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22).

The repository includes a [good first issue template](.github/ISSUE_TEMPLATE/good-first-issue.md) for maintainers.

## Pull requests

A useful pull request includes:

- a short summary of the user-visible change;
- tests or commands run, including any limitation;
- screenshots or a short recording for UI changes;
- notes about migrations, configuration, or platform-specific behavior.

Maintainers may ask for a smaller scope or an additional test before merging. That is part of keeping Mint approachable for future contributors.

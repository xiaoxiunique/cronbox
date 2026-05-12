# Open Source Checklist

This checklist tracks the work needed to make CronBox comfortable to publish and maintain as an open source project.

## Done

- README describes the project, current status, development flow, CLI, packaging, agent tasks, contributing, and license.
- MIT license is present in `LICENSE`.
- MIT is confirmed as the project license.
- `package.json` and `src-tauri/Cargo.toml` include license and repository metadata.
- `CONTRIBUTING.md` documents local setup, tests, and contribution expectations.
- `SECURITY.md` defines private vulnerability reporting expectations.
- `CODE_OF_CONDUCT.md` sets collaboration standards.
- `CHANGELOG.md` exists with an Unreleased section.
- GitHub issue templates and pull request template are present.
- Dependabot is configured for npm, Cargo, and GitHub Actions.
- CI workflow runs frontend build, Rust format check, and Rust tests.
- Packaging workflow builds macOS, Linux, and Windows installers on main, manual runs, and version tags.
- Static project site exists under `sites/cronbox`.

## Before Public Launch

- Keep GitHub private vulnerability reporting disabled for now.
- Add repository topics such as `tauri`, `scheduler`, `cron`, `menubar`, `local-first`, `codex`, and `claude`.
- Make the repository public when ready.
- Publish a first version tag, for example `v0.1.0`, after reviewing the generated installers.
- Decide whether unsigned macOS and Windows builds are acceptable for early releases.
- If distributing broadly, configure code signing and macOS notarization.
- Add screenshots or a short demo GIF to the README.
- Add a public project site deployment for `sites/cronbox` through Cloudflare Pages.
- Review generated CLI help before release and keep README examples in sync.

## Nice To Have

- Add architecture notes for the scheduler, job database, executor, and menu bar runtime.
- Add a release checklist for version bumps, changelog updates, tag creation, and installer validation.
- Add a test fixture directory for script scanning behavior.
- Add smoke tests for generated Codex and Claude task scripts where the CLIs are available.
- Add signed update support if automatic updates become part of the product.

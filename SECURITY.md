# Security Policy

CronBox runs local scripts and coding-agent tasks, so security issues can affect the user's filesystem and developer environment.

## Reporting a Vulnerability

Please do not open public issues for vulnerabilities.

Use GitHub private vulnerability reporting if it is enabled for the repository. If it is not available, contact the maintainer through GitHub and ask for a private reporting channel.

Include:

- affected version or commit
- operating system
- steps to reproduce
- impact
- any relevant logs or generated scripts

## Scope

Security-sensitive areas include:

- command execution
- script scanning and argument handling
- generated Codex or Claude task scripts
- filesystem access and allowed directories
- CLI installation or symlink handling
- update, packaging, and release workflows

## Expectations

The maintainers will acknowledge valid reports as soon as possible, investigate the impact, and coordinate a fix before public disclosure when appropriate.

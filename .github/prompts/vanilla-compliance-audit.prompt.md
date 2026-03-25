---
agent: 'vanilla-compliance-audit'
description: 'Fix vanilla compliance issues in the Oxidized codebase.'
---

# Vanilla Compliance Audit

Audit the entire Oxidized codebase for behavioral divergence from the vanilla Minecraft server, then fix every finding.

## Instructions

1. Use `@vanilla-auditor` to audit the codebase. It will discover implemented systems, compare them against the vanilla Java source, and return findings.
2. Review the audit report.
3. Plan fixes grouped by severity (🔴 → 🟡 → 🔵 → ⚪), one commit per logical fix.
4. Implement fixes yourself in that order. Add tests for every fix. Run `cargo check --workspace && cargo test --workspace` after each.

## What to Skip

- Systems in planned/future phases — `@vanilla-auditor` already filters these
- Architecture differences (ECS vs OOP, Tokio vs Netty) — only wire behavior matters
- Commands documented as stubs
- Style or formatting — only behavioral issues

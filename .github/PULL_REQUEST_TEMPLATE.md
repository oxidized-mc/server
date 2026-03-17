## Summary

<!-- One-sentence description of what this PR does. -->

## Motivation

<!-- Why is this change needed? Link to relevant issue(s): Closes #xxx -->

## Changes

<!-- Bullet list of what changed and in which crate(s). -->

- 
- 

## Reference

<!-- Did you check the vanilla Java reference? Paste the relevant class/method. -->

<details>
<summary>Java reference (if applicable)</summary>

```java
// mc-server-ref/decompiled/net/minecraft/...
```

</details>

## Testing

<!-- How did you test this? Unit tests added? Manual client test? -->

- [ ] Unit tests added / updated
- [ ] Tested with vanilla 26.1 client (if applicable)
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] `cargo fmt --check` passes

## Quality Checklist

<!-- Standard code quality checks. All must pass. -->

- [ ] Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/)
- [ ] Public API has documentation (`///` doc comments)
- [ ] No `unwrap()` / `expect()` in production paths (use `?` or proper error handling)
- [ ] No hardcoded magic numbers — use named constants
- [ ] CHANGELOG.md updated if user-visible change

## ADR Compliance

<!-- Verify this change respects existing architecture decisions. -->

- [ ] Reviewed relevant ADRs linked in the phase document
- [ ] Implementation follows the decisions in those ADRs
- [ ] Crate dependency rules are respected (no upward imports)
- [ ] Error handling follows [ADR-002](docs/adr/adr-002-error-handling.md)

## Continuous Improvement

<!-- Every PR is an opportunity to make the project better. -->

- [ ] **Checked:** Are any existing ADRs outdated given this change?
- [ ] **Checked:** Could any existing patterns be improved?
- [ ] **Checked:** Are there stale references (renamed items, moved files, changed APIs)?
- [ ] **Checked:** Were any learnings discovered that should be added to [memories.md](.github/memories.md)?
- [ ] Any identified improvements are recorded (memories.md, new issue, or new ADR)

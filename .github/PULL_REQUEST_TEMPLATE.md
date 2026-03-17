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

## Checklist

- [ ] Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/)
- [ ] Public API has documentation (`///` doc comments)
- [ ] No `unwrap()` / `expect()` in production paths (use `?` or proper error handling)
- [ ] No hardcoded magic numbers — use named constants
- [ ] CHANGELOG.md updated if user-visible change

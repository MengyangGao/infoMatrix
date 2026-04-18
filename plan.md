# Objective
Make the release pipeline behave like a real software distribution pipeline: tagged releases must only publish installable, trustable artifacts, and the macOS build must not fall back to ad-hoc signing in public releases.

# User Value
Users who download a tagged release should get a build that launches normally and matches the promised platform support. The repository should stop producing public release assets that look official but fail Gatekeeper or are otherwise unusable.

# Constraints
- Keep the change focused on release gating, artifact selection, and macOS trust/signing behavior.
- Do not weaken the current local development or smoke-test workflow.
- Preserve fast CI feedback for non-tag builds and manual workflow dispatches.
- Avoid publishing misleading public artifacts such as macOS ad-hoc builds or iOS simulator zips.

# Assumptions
- [ASSUMPTION] The current public release is not acceptable as-is because it was built without a production Apple signing/notarization chain.
- [ASSUMPTION] GitHub Release assets should only contain installable user-facing binaries for the platforms we can truly distribute today.
- [ASSUMPTION] iOS App Store distribution is a later phase, so public GitHub Releases should not pretend to be a direct iOS delivery channel.
- [ASSUMPTION] The repo may still need separate secrets setup before the next tagged release can succeed.

# Affected Files
- .github/workflows/release.yml
- docs/release.md
- docs/targets.md
- README.md
- apps/apple/macOS/README.md
- apps/apple/iOS/README.md
- plan.md

# Steps
1. Tighten the release workflow so tag builds fail immediately if the Apple signing/notarization prerequisites are missing.
2. Remove non-installable iOS simulator assets from public GitHub Release publishing.
3. Keep the local smoke workflow available for manual runs, but make tagged releases official-only.
4. Update the release docs so they describe the actual public distribution policy.
5. Run the release gate and verify that the code still builds and tests cleanly.

# Validation
- `./tooling/scripts/release_check.sh`
- `cargo test --workspace`
- Review the GitHub workflow YAML for the new tag-release rules.

# Risks
- The stricter release gate will cause tagged releases to fail until the Apple signing and Android release-signing secrets are actually configured.
- If the public release asset list is reduced too far, users may lose an artifact they relied on previously.
- The workflow may still be correct while the GitHub repository secrets remain incomplete, so the next release attempt could still stop at the gate rather than publishing.

# Rollback Notes
- If the stricter release gate blocks intended smoke testing, revert only the tag-only failure path and keep the artifact filtering.
- If the public artifact list is too aggressive, re-add only the specific installable asset types, not the simulator-only ones.

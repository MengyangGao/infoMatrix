# Objective
Make tagged GitHub Releases publish successfully again by removing the hard dependency on an iOS `.ipa` that this release pipeline does not produce in the current smoke-release mode.

# User Value
Users can download the macOS release build from GitHub Releases without the publish job failing on a missing iOS device artifact. The release workflow stays aligned with the actual artifacts the build produces.

# Constraints
- Keep the fix narrow and release-focused.
- Do not reintroduce Android into tagged releases.
- Preserve the current smoke/public release behavior for Apple and desktop platforms.
- Keep release documentation truthful about what tagged releases contain.
- Add or update checks so future publish mismatches are caught early.

# Assumptions
- [ASSUMPTION] The current tagged release path is intended to publish macOS, Windows, Linux, and iOS simulator artifacts, not a device `.ipa`.
- [ASSUMPTION] The missing `.ipa` is a packaging-asset mismatch, not a build break in the iOS job itself.
- [ASSUMPTION] Android remains intentionally excluded from tagged releases for now.

# Affected Files
- .github/workflows/release.yml
- README.md
- docs/release.md
- docs/targets.md
- plan.md

# Steps
1. Update the release workflow so the publish step no longer requires an iOS `.ipa` pattern that is not produced.
2. Align the public release docs and README with the actual iOS asset set.
3. Re-run local validation on the workflow YAML and surrounding docs.
4. Confirm the workflow change is minimal and does not re-enable Android publishing.

# Validation
- `bash -n` or YAML parse check for `.github/workflows/release.yml`
- `git diff --check`
- Search/spot-check release docs for iOS asset consistency

# Risks
- If the iOS job later starts producing `.ipa` again, the release docs may under-describe the available asset unless updated.
- Removing the `.ipa` from publication may surprise anyone expecting device installs from the GitHub Release page.
- The workflow could still fail later for unrelated asset mismatches if other patterns drift.

# Rollback Notes
- If the change breaks release publication expectations, restore the `.ipa` pattern and update the iOS packaging script/workflow together instead of partially reverting only one side.

---
id: TASK-0038
title: 'Fix feed display (unknown author, missing commentary)'
status: Done
assignee:
  - '@claude'
created_date: '2026-03-24 09:22'
updated_date: '2026-03-24 10:12'
labels: []
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Feed endpoint returns data but the human-readable display shows 'unknown author' and no commentary text. The JSON response structure doesn't match the expected field paths (actor.name.text, commentary.text.text). Inspect the saved fixture at secrets/feed_response.json to find the actual field paths and fix the display code.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Feed list shows author names
- [x] #2 Feed list shows post text/commentary
- [x] #3 Like and comment counts display correctly
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Root cause: print_feed_item accessed fields directly on the element (e.g. element.actor.name.text) but the real response nests the UpdateV2 payload under element.value["com.linkedin.voyager.feed.render.UpdateV2"]. This is LinkedIn's Rest.li union type encoding.

Fix: Added unwrap step at the top of print_feed_item that dereferences through value["com.linkedin.voyager.feed.render.UpdateV2"] before accessing actor, commentary, and socialDetail. Falls back to the element itself if the wrapper is absent.

Validated against secrets/feed_response.json fixture — all 3 items show correct author names, commentary text, and like/comment counts.

All e2e tests pass (35 unit + 2 smoke + clippy + fmt).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Fixed feed display showing "unknown author" and missing commentary/counts.

Root cause: print_feed_item in linkedin-cli/src/main.rs accessed actor, commentary, and socialDetail fields directly on each feed element, but the actual LinkedIn API response nests the UpdateV2 payload inside element.value["com.linkedin.voyager.feed.render.UpdateV2"] (Rest.li union encoding).

Change: Added a single unwrap step at the top of print_feed_item that dereferences through the Rest.li union wrapper before field access, with fallback to the element itself for forward-compatibility.

File changed: linkedin/linkedin-cli/src/main.rs (print_feed_item function)

Tested: cargo build, just e2e (35 unit + 2 smoke + clippy + fmt all pass), validated field extraction against secrets/feed_response.json fixture.
<!-- SECTION:FINAL_SUMMARY:END -->

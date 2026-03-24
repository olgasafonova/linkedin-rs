---
id: TASK-0038
title: 'Fix feed display (unknown author, missing commentary)'
status: To Do
assignee: []
created_date: '2026-03-24 09:22'
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
- [ ] #1 Feed list shows author names
- [ ] #2 Feed list shows post text/commentary
- [ ] #3 Like and comment counts display correctly
<!-- AC:END -->

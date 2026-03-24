---
id: TASK-0011
title: Analyze React Native JS bundle for API calls
status: Done
assignee:
  - '@claude'
created_date: '2026-03-23 23:00'
updated_date: '2026-03-24 05:55'
labels:
  - phase3
  - static-analysis
  - react-native
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Decompile and analyze the Hermes bytecode in assets/index.android.bundle to find API calls originating from the React Native layer. The RN layer may use different endpoints or patterns than the native layer. Use hermes-dec or hbcdump to decompile the Hermes bytecode, then search for fetch/XMLHttpRequest calls, API URLs, and data models. Some newer features are built in RN and may not have native Java equivalents.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 JS bundle successfully decompiled from Hermes bytecode
- [x] #2 API calls from RN layer identified and documented
- [x] #3 Any RN-only endpoints added to the endpoint catalog
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Bundle is readable minified JS (not Hermes bytecode). 2MB file. Analyzing for API calls.

Bundle is readable minified JS, not Hermes bytecode -- no decompilation needed. AC#1 adapted: bundle was directly readable.

Found 7 Zephyr-only API endpoints and 1 shared Voyager endpoint. All documented in re/js_bundle_analysis.md. RN-only endpoints added to analysis (section 7.2).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
JS bundle is readable minified JavaScript (not Hermes bytecode), so no decompilation was needed. Found 994 RN modules in 2MB bundle. Identified 8 API endpoints used by the RN layer (7 Zephyr-only, 1 shared Voyager). The RN layer handles Social Hiring, lightweight job browsing, content creation, and user info -- focused on China market features. Notable: RN fetch calls omit X-RestLi-Protocol-Version header, suggesting Zephyr endpoints are REST wrappers. Also documented HMS configs, CUCC carrier integration, CDN URLs, and CodePush OTA capability. Full analysis in re/js_bundle_analysis.md.
<!-- SECTION:FINAL_SUMMARY:END -->

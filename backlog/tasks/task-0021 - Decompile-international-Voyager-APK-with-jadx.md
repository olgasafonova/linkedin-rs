---
id: TASK-0021
title: Decompile international Voyager APK with jadx
status: Done
assignee:
  - '@claude'
created_date: '2026-03-24 07:02'
updated_date: '2026-03-24 07:19'
labels: []
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Run jadx on linkedin_intl_4.1.1183.apk to get the international variant's decompiled source. This is the build we want to target for the Rust client (/voyager/api/ not /zephyr/api/).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 International APK decompiled to decompiled/jadx_intl/
- [x] #2 Decompilation errors documented
- [x] #3 Top-level package differences from Zephyr noted
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Run jadx on linkedin_intl_4.1.1183.apk targeting decompiled/jadx_intl/
2. Capture error count and total class count from jadx output
3. List top-level packages and compare with Zephyr (decompiled/jadx/)
4. Verify key classes: Routes.java, CronetNetworkEngine, liauthlib, Pegasus models
5. Document findings in implementation notes
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
jadx completed on linkedin_intl_4.1.1183.apk (90MB, 41542 classes).

Results:
- 38,333 Java files written to decompiled/jadx_intl/
- 202 errors (all NullPointerException on codeCache -- heap exhaustion during save phase)
- Error rate: 0.49% (202/41542)
- Output size: 383MB on disk

Top-level packages (intl): android, androidx, avro, coil3, com, _COROUTINE, curtains, cz, dagger, dalvik, io, J, java9, javax, jp, kotlin, kotlinx, li, me, net, okhttp3, okio, opentovolunteer, org, pegasus, proto, radiography, retrofit2, si

com.linkedin.android sub-packages: 159 (intl) vs 93 (Zephyr)
- 88 packages unique to intl (ads, antiabuse, conversations, hiring, live, notifications, etc.)
- 23 packages unique to Zephyr (zephyr, reactnative, flagship, jobs, wxapi, etc.)
- Zephyr has cn/ and bolts/ at top-level; intl has avro/, coil3/, proto/, retrofit2/ etc.

Key classes verified present:
- Routes.java at com.linkedin.android.infra.shared.Routes
- CronetNetworkEngine at com.linkedin.android.networking.engines.cronet.CronetNetworkEngine
- liauthlib at com.linkedin.android.liauthlib/ (LiAuth, LiAuthImpl, login, sso, etc.)
- Pegasus models at com.linkedin.android.pegasus.dash.gen.voyager.dash/
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Decompiled international LinkedIn Voyager APK (linkedin_intl_4.1.1183.apk, 90MB) with jadx.

Results:
- 41,542 total classes, 38,333 Java files written to decompiled/jadx_intl/ (383MB on disk)
- 202 decompilation errors (0.49% error rate) -- all NullPointerException on codeCache due to heap exhaustion during save phase; not a code quality issue
- International build has 159 sub-packages under com.linkedin.android vs 93 for Zephyr
- 88 packages unique to intl (ads, hiring, live, notifications, conversations, etc.); 23 unique to Zephyr (zephyr, reactnative, flagship, wxapi, etc.)
- Top-level: intl adds avro, coil3, proto, retrofit2, radiography; Zephyr has cn/, bolts/ (China-specific)
- All key classes verified: Routes.java, CronetNetworkEngine, liauthlib (LiAuth/LiAuthImpl/login/sso), Pegasus dash models under voyager.dash/
<!-- SECTION:FINAL_SUMMARY:END -->

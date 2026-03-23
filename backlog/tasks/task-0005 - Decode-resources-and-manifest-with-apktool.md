---
id: TASK-0005
title: Decode resources and manifest with apktool
status: Done
assignee:
  - '@claude'
created_date: '2026-03-23 22:32'
updated_date: '2026-03-23 22:51'
labels: []
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Run apktool to decode AndroidManifest.xml, resources, and other non-code assets. Document activities, services, permissions, and intent filters.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 apktool decode output in decompiled/apktool/
- [x] #2 AndroidManifest.xml analyzed and documented
- [x] #3 Key permissions and components listed in re/
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Run apktool d on APK to decode resources
2. Parse AndroidManifest.xml for activities, services, receivers, providers, permissions, intent-filters, meta-data
3. Write structured analysis to re/manifest_analysis.md
4. Check ACs and mark done
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- apktool 2.12.1 decoded APK successfully to decompiled/apktool/ (5 DEX files baksmaled)
- AndroidManifest.xml is 1809 lines, thoroughly analyzed
- Manifest analysis written to re/manifest_analysis.md covering: permissions, activities, services, providers, receivers, deep links, meta-data/API keys, auth architecture, push strategy, network security
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Decoded LinkedIn Android APK (v6.1.1, 53MB) using apktool 2.12.1 and performed thorough AndroidManifest.xml analysis.

Key findings:
- Three auth entry points identified: LaunchActivity, LiThirdPartyAuthorizeActivity, LiOneClickLoginInitActivity
- Two exported SSO services (OAuthService, LiSSOService) that provide tokens to other apps
- Auth library in com.linkedin.android.liauthlib with challenge/CAPTCHA handling
- Real-time subscriptions via SystemSubscriptions content provider
- Multi-provider push strategy: FCM + GeTui (Huawei, Oppo, Vivo, Xiaomi)
- Two-layer deep link architecture with linkedin:// scheme and 80+ country subdomains
- Cleartext traffic allowed globally, debug builds trust user CAs (good for MITM)
- Chinese market features (Chitu, Wukong, WeChat mini program, Shanyan carrier login)
- React Native present but likely not critical for API RE

Output: decompiled/apktool/ (full decode), re/manifest_analysis.md (structured analysis)
<!-- SECTION:FINAL_SUMMARY:END -->

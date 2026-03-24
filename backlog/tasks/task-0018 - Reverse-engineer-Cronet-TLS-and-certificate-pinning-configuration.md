---
id: TASK-0018
title: Reverse-engineer Cronet TLS and certificate pinning configuration
status: To Do
assignee: []
created_date: '2026-03-24 06:29'
labels: []
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Read CronetNetworkEngine classes to determine if certificate pinning is configured in the Java layer, what domains are pinned, and what TLS configuration is applied. Document implications for the Rust client's TLS setup. Package: com.linkedin.android.networking.engines.cronet
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Cronet pinning configuration documented (or confirmed absent from Java layer)
- [ ] #2 TLS version and cipher requirements documented
- [ ] #3 Recommendations for Rust TLS backend written
- [ ] #4 Findings written to re/tls_configuration.md
<!-- AC:END -->

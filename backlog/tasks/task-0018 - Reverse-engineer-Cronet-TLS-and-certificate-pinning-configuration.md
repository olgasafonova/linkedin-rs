---
id: TASK-0018
title: Reverse-engineer Cronet TLS and certificate pinning configuration
status: Done
assignee:
  - '@claude'
created_date: '2026-03-24 06:29'
updated_date: '2026-03-24 06:42'
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
- [x] #1 Cronet pinning configuration documented (or confirmed absent from Java layer)
- [x] #2 TLS version and cipher requirements documented
- [x] #3 Recommendations for Rust TLS backend written
- [x] #4 Findings written to re/tls_configuration.md
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Read CronetNetworkEngine and CronetNetworkEngineWithoutExecution to find engine init
2. Read CronetEngineBuilderImpl for PKP/QUIC/TLS settings
3. Read CronetUrlRequestContext for native bridge parameters
4. Check network-security-config XML for pin-set elements
5. Check OkHttp CertificatePinner for LinkedIn-specific pins
6. Read X509Util and AndroidNetworkLibrary for cert verification flow
7. Confirm Cronet version from ImplVersion and native .so files
8. Document all findings in re/tls_configuration.md
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- CronetEngineBuilderImpl.mPkps is empty LinkedList, never populated
- network-security-config (res/xml/p.xml) has no pin-set elements
- OkHttp CertificatePinner present but no LinkedIn-specific pins configured
- Certificate verification delegates to Android system TrustManager via X509Util
- Cronet version confirmed: 83.0.4103.83 (from ImplVersion.java and .so filenames)
- Engine init enables HTTP/2 and QUIC, uses BoringSSL for TLS
- No TLS version or cipher configuration in Java layer -- all baked into native BoringSSL
- Recommended Rust TLS backend: reqwest with boring-tls feature
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Reverse-engineered Cronet TLS and certificate pinning configuration from the LinkedIn Android APK.

Key findings:
- No certificate pinning in Java layer: CronetEngineBuilderImpl.mPkps is empty, network-security-config has no pin-set elements, OkHttp CertificatePinner not configured with LinkedIn pins
- Chromium 83 native layer handles all TLS via BoringSSL; no Java-layer TLS configuration exists
- Engine enables HTTP/2 + QUIC; TLS 1.3 with BoringSSL defaults (matching Chrome 83 cipher suite order)
- Certificate verification delegates to Android system TrustManager (no custom pinning)
- Recommended Rust TLS backend: reqwest with boring-tls (same BoringSSL library Chrome uses), avoiding rustls/native-tls due to fingerprint mismatch
- Documented JA3/JA4 fingerprint characteristics, HTTP/2 SETTINGS values, and specific configuration requirements

Output: re/tls_configuration.md
<!-- SECTION:FINAL_SUMMARY:END -->

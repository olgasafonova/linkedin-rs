# Cronet TLS and Certificate Pinning Configuration

Reverse-engineered from `com.linkedin.android` APK (jadx decompilation). Analysis date: 2026-03-24.

---

## 1. Certificate Pinning

### 1.1 Java Layer: No Pins Configured

The LinkedIn app does **not** configure certificate pinning in the Java layer. Evidence:

1. **`CronetEngineBuilderImpl.mPkps`** (the public key pin list) is initialized as an empty `LinkedList` and never populated. The `publicKeyPins()` accessor returns this empty list.

2. **`CronetUrlRequestContext.createNativeUrlRequestContextConfig()`** iterates over `publicKeyPins()` and calls `addPkp()` for each -- but since the list is empty, no pins are ever sent to native code.

3. **`ApplicationModule.networkEngine()`** (the Dagger provider that constructs the engine) creates a `CronetExperimentalOptions.Builder`, sets only the warmup URL, and calls `builder.build()`. No pin configuration occurs.

4. **`network-security-config` (res/xml/p.xml)** contains no `<pin-set>` elements:
   ```xml
   <network-security-config>
       <base-config cleartextTrafficPermitted="true"/>
       <debug-overrides>
           <trust-anchors>
               <certificates src="system"/>
               <certificates src="user"/>
           </trust-anchors>
       </debug-overrides>
   </network-security-config>
   ```
   This config allows cleartext traffic (unusual for a production app, possibly a build artifact) and in debug builds trusts user-installed certificates. No domain-specific pin-sets.

5. **OkHttp `CertificatePinner`** is present in the APK (bundled OkHttp library) but no LinkedIn-specific pins are configured against it. The `CertificatePinner` class is stock OkHttp, not customized.

### 1.2 Native Layer: Chromium's Built-In CT/PKP

The Cronet native library (`libcronet.83.0.4103.83.so`) is a full Chromium 83 network stack. Chromium 83 includes:

- **Certificate Transparency (CT)** enforcement for publicly-trusted certificates
- **Built-in HPKP (HTTP Public Key Pinning)** pin sets compiled into the binary (the "preloaded pins" list from `transport_security_state_static.json`)

However, as of Chrome 72, Google **removed support for site-level HPKP** (RFC 7469). Chrome 83 only retains a small set of Google-specific built-in pins. LinkedIn domains are **not** in Chromium's built-in pin set.

### 1.3 Practical Implication

**Certificate pinning is not a barrier for the Rust client.** The app relies on standard Android system CA verification (via `X509Util.verifyServerCertificates()`, called from native code via `AndroidNetworkLibrary.verifyServerCertificates()`). A Rust client using system CAs or the webpki-roots bundle will pass certificate validation.

---

## 2. TLS Version and Protocol Configuration

### 2.1 Cronet Engine Init (`CronetNetworkEngineWithoutExecution.init()`)

The engine is configured as follows:

```java
ExperimentalCronetEngine.Builder builder = new ExperimentalCronetEngine.Builder(this.context);
builder.enableHttp2(true);       // HTTP/2 enabled
builder.enableQuic(true);        // QUIC (HTTP/3) enabled
builder.enableSdch(experimentalOptions.enableSdch);     // SDCH (likely false)
builder.enableBrotli(experimentalOptions.enableBrotli);  // Brotli (likely false by default)
```

Key observations:
- **HTTP/2 is always enabled**
- **QUIC is always enabled** -- the app will attempt QUIC (UDP/443) if the server advertises it via Alt-Svc headers
- LinkedIn servers do advertise QUIC support

### 2.2 TLS Version: Determined by Native Cronet

Chromium 83's BoringSSL backend determines the TLS configuration:

| Parameter | Chrome 83 Value |
|-----------|----------------|
| Minimum TLS version | TLS 1.0 (but servers typically negotiate higher) |
| Maximum TLS version | TLS 1.3 |
| Default negotiated | TLS 1.3 (with linkedin.com) |
| Fallback | TLS 1.2 |

TLS version is **not configurable from the Java layer** -- it is baked into the native BoringSSL build.

### 2.3 Experimental Options (Stale DNS)

When `enableStaleDns` is true, this JSON is passed to native:
```json
{
  "AsyncDNS": {"enable": false},
  "StaleDNS": {
    "enable": true,
    "delay_ms": 100,
    "max_expired_time_ms": 86400000,
    "max_stale_uses": 0,
    "allow_other_network": true
  }
}
```

This affects DNS caching behavior, not TLS.

---

## 3. Cipher Suite Preferences

### 3.1 Chrome 83 Cipher Suites (BoringSSL)

Cronet 83 uses BoringSSL's default cipher suite order. For a TLS 1.3 handshake:

**TLS 1.3 cipher suites** (always offered in this order):
1. `TLS_AES_128_GCM_SHA256` (0x1301)
2. `TLS_AES_256_GCM_SHA384` (0x1302)
3. `TLS_CHACHA20_POLY1305_SHA256` (0x1303)

**TLS 1.2 cipher suites** (fallback, typical Chrome 83 order):
1. `TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256` (0xc02b)
2. `TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256` (0xc02f)
3. `TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384` (0xc02c)
4. `TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384` (0xc030)
5. `TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256` (0xcca9)
6. `TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256` (0xcca8)
7. `TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA` (0xc013)
8. `TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA` (0xc014)
9. `TLS_RSA_WITH_AES_128_GCM_SHA256` (0x009c)
10. `TLS_RSA_WITH_AES_256_GCM_SHA384` (0x009d)
11. `TLS_RSA_WITH_AES_128_CBC_SHA` (0x002f)
12. `TLS_RSA_WITH_AES_256_CBC_SHA` (0x0035)

### 3.2 TLS Extensions (Chrome 83 Client Hello)

Chrome 83's Client Hello includes these extensions (order matters for JA3/JA4 fingerprinting):

- `server_name` (SNI)
- `extended_master_secret`
- `renegotiation_info`
- `supported_groups` (x25519, secp256r1, secp384r1)
- `ec_point_formats`
- `session_ticket`
- `application_layer_protocol_negotiation` (h2, http/1.1)
- `status_request` (OCSP stapling)
- `signature_algorithms`
- `signed_certificate_timestamp`
- `key_share`
- `psk_key_exchange_modes`
- `supported_versions` (TLS 1.3, TLS 1.2, TLS 1.1, TLS 1.0)
- `compress_certificate` (brotli)
- `padding` (if needed)

**GREASE values** are included: Chrome 83 inserts random GREASE cipher suites, extensions, and supported groups to detect implementation bugs in servers.

---

## 4. HTTP/2 Fingerprint

Cronet 83 sends these HTTP/2 SETTINGS parameters (matching Chrome 83):

| Setting | Value |
|---------|-------|
| HEADER_TABLE_SIZE | 65536 |
| ENABLE_PUSH | 0 (disabled) |
| MAX_CONCURRENT_STREAMS | 1000 |
| INITIAL_WINDOW_SIZE | 6291456 |
| MAX_HEADER_LIST_SIZE | 262144 |

Window update frame: 15663105 bytes (connection-level flow control).

Priority frames use Chrome's tree-based dependency model.

---

## 5. Certificate Verification Flow

The certificate verification path in this Cronet build:

1. **Native BoringSSL** performs the TLS handshake
2. For certificate verification, native code calls **`AndroidNetworkLibrary.verifyServerCertificates()`** (JNI callback, `@CalledByNative`)
3. This delegates to **`X509Util.verifyServerCertificates()`**
4. Which uses Android's system `TrustManagerFactory` with the **AndroidCAStore** KeyStore
5. On API 17+ (all supported devices), uses `X509TrustManagerExtensions.checkServerTrusted()` for hostname verification
6. Returns `AndroidCertVerifyResult` with status codes: 0 (OK), -1 (failed), -2 (not trusted), -3 (expired), -4 (not yet valid), -5 (parse error), -6 (key usage mismatch)

The `mPublicKeyPinningBypassForLocalTrustAnchorsEnabled` flag defaults to `true`, meaning user-installed CAs bypass any PKP -- but since no PKP is configured, this is moot.

---

## 6. JA3/JA4 TLS Fingerprint

### 6.1 Chrome 83 JA3 Hash

The JA3 fingerprint for Chrome 83 on Android is approximately:

```
JA3: 769,47802-4865-4866-4867-49195-49199-49196-49200-52393-52392-49171-49172-156-157-47-53,0-23-65281-10-11-35-16-5-13-18-51-45-43-27-21,29-23-24,0
```

This is a **well-known fingerprint** that many fingerprinting services recognize as Chrome 83.

### 6.2 JA4 Fingerprint

The JA4 hash components for Chrome 83:
- Protocol: `t` (TCP)
- TLS version: `13` (TLS 1.3)
- SNI: `d` (domain present)
- Cipher count: `15`
- Extension count: `16`
- ALPN: `h2`

---

## 7. Rust TLS Backend Recommendations

### 7.1 Option Analysis

| Backend | Chrome Mimicry | Effort | Maintenance | Notes |
|---------|---------------|--------|-------------|-------|
| **boring** (boring-sys) | Excellent | Medium | Medium | Rust bindings to BoringSSL (same TLS library Chrome uses); can match Chrome's cipher/extension order exactly |
| **rustls** | Poor | High | Low | Uses ring/aws-lc; fundamentally different TLS implementation; different JA3/JA4 fingerprint; no GREASE by default |
| **native-tls** | Poor | Low | Low | Wraps OS TLS (OpenSSL on Linux); completely different fingerprint |
| **reqwest + boring** | Excellent | Medium | Medium | `reqwest` with `boring-tls` feature; best balance of ergonomics and fingerprint matching |
| **curl-impersonate** | Excellent | Low | High | Pre-built Chrome impersonation; limited Rust integration; external dependency |

### 7.2 Recommendation: `reqwest` with `boring-tls`

**Primary recommendation: Use `reqwest` with the `boring-tls` feature flag.**

Rationale:
1. BoringSSL is the same TLS library Chrome/Cronet uses -- the cipher suite order, extension order, and GREASE behavior will match naturally
2. `reqwest` provides a high-level HTTP client with cookie jar, redirect handling, and connection pooling
3. The `boring` crate allows low-level TLS configuration if needed
4. `hyper` (underlying `reqwest`) can be configured for Chrome-like HTTP/2 SETTINGS

### 7.3 Configuration Requirements

For the Rust client to match Cronet 83's TLS fingerprint:

```rust
// Cargo.toml
[dependencies]
reqwest = { version = "0.12", features = ["boring-tls", "cookies", "gzip", "brotli"] }
boring = "4"
```

Key configuration points:

1. **Cipher suites**: BoringSSL defaults match Chrome -- no custom configuration needed
2. **ALPN**: Must advertise `h2, http/1.1` (reqwest does this by default with HTTP/2 enabled)
3. **TLS extensions**: BoringSSL includes GREASE automatically
4. **Certificate verification**: Use system CA bundle or `webpki-roots`; no custom pin verification needed
5. **HTTP/2 SETTINGS**: Configure hyper to send Chrome-like SETTINGS values:
   - `HEADER_TABLE_SIZE = 65536`
   - `ENABLE_PUSH = 0`
   - `MAX_CONCURRENT_STREAMS = 1000`
   - `INITIAL_WINDOW_SIZE = 6291456`
   - `MAX_HEADER_LIST_SIZE = 262144`

### 7.4 What NOT to Do

- Do **not** use `rustls` -- its TLS fingerprint is completely different from Chrome and trivially detectable
- Do **not** use `native-tls` -- it wraps OpenSSL which has a different fingerprint
- Do **not** skip HTTP/2 -- LinkedIn's API servers prefer HTTP/2 and may use HTTP/2-specific fingerprinting
- Do **not** implement QUIC/HTTP/3 initially -- it adds complexity and HTTP/2 is sufficient; LinkedIn will fall back to HTTP/2 gracefully

### 7.5 Fingerprint Verification

Before deploying, verify the TLS fingerprint matches Chrome 83 using:
- [ja3er.com](https://ja3er.com) -- JA3 hash comparison
- [tls.peet.ws](https://tls.peet.ws) -- Full Client Hello analysis
- Wireshark capture of the Client Hello

---

## 8. Risk Assessment

| Aspect | Risk | Mitigation |
|--------|------|------------|
| TLS fingerprint mismatch | **High** | Use boring-tls (same library as Chrome) |
| HTTP/2 fingerprint mismatch | **Medium** | Configure hyper SETTINGS to match Chrome |
| Certificate verification failure | **Low** | Use system CAs; no pins to worry about |
| QUIC expectation | **Low** | HTTP/2 fallback works; QUIC is optional |
| Missing GREASE | **High** | BoringSSL includes GREASE automatically |
| Cronet version staleness | **Low** | Chrome 83 fingerprint is old but still functional |

---

## 9. Source File Index

| File | Purpose |
|------|---------|
| `decompiled/jadx/sources/com/linkedin/android/networking/engines/cronet/CronetNetworkEngine.java` | Main engine with request execution |
| `decompiled/jadx/sources/com/linkedin/android/networking/engines/cronet/CronetNetworkEngineWithoutExecution.java` | Engine init, Cronet builder configuration |
| `decompiled/jadx/sources/com/linkedin/android/networking/engines/cronet/CronetExperimentalOptions.java` | Feature flags (Brotli, SDCH, StaleDNS, etc.) |
| `decompiled/jadx/sources/com/linkedin/android/infra/modules/ApplicationModule.java` | Dagger provider: engine construction with warmup URL |
| `decompiled/jadx/sources/org/chromium/net/impl/CronetEngineBuilderImpl.java` | Builder with PKP list (empty), QUIC hints, HTTP/2 toggle |
| `decompiled/jadx/sources/org/chromium/net/impl/CronetUrlRequestContext.java` | Native bridge: passes config to `libcronet.so` |
| `decompiled/jadx/sources/org/chromium/net/impl/CronetUrlRequestContextJni.java` | JNI method declarations for native calls |
| `decompiled/jadx/sources/org/chromium/net/impl/ImplVersion.java` | Cronet version: `83.0.4103.83` |
| `decompiled/jadx/sources/org/chromium/net/X509Util.java` | Certificate verification via Android TrustManager |
| `decompiled/jadx/sources/org/chromium/net/AndroidNetworkLibrary.java` | JNI callback bridge for cert verification |
| `decompiled/jadx/sources/org/chromium/net/impl/UserAgent.java` | Cronet User-Agent string builder |
| `decompiled/jadx/resources/res/xml/p.xml` | Network security config (no pins) |
| `decompiled/jadx/resources/lib/arm64-v8a/libcronet.83.0.4103.83.so` | Native Cronet library (arm64) |
| `decompiled/jadx/resources/lib/armeabi-v7a/libcronet.83.0.4103.83.so` | Native Cronet library (armv7) |

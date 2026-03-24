---
id: TASK-0017
title: 'Document serialization edge cases (null handling, timestamps, unions)'
status: Done
assignee:
  - '@claude'
created_date: '2026-03-24 06:29'
updated_date: '2026-03-24 06:51'
labels: []
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Examine Pegasus RecordTemplate/UnionTemplate serialization to determine: null vs absent field distinction, empty collection handling, timestamp format (epoch seconds vs ms), deep nested union discriminator format, and custom serializers. Package: com.linkedin.data.lite, com.linkedin.android.pegasus

Findings summary:
- Null vs absent: two-flag system (hasXxx + value). Both absent-from-JSON and JSON-null map to hasXxx=false. Omit absent fields on serialization.
- Empty collections: three states (absent, explicitly-empty, non-empty). Builder tracks explicit-empty via separate boolean. Normalize to empty Vec in Rust.
- Timestamps: all epoch milliseconds (long). No conversion code anywhere. Fields: createdAt, expiresAt, lastModified, created, deleted.
- Union discriminator: FQN string as single JSON key (e.g. "com.linkedin.voyager.feed.ShareArticle"). Nesting is natural -- each union level adds one {fqn: value} wrapper.
- Zero-member unions are valid (no validation). Only >1 member throws.
- 5 serializer implementations: JacksonJsonGenerator, JSONObjectGenerator, RawDataGenerator, ProtobufGenerator, FissionProtobufGenerator.
- Coercer system for type conversions (UrnCoercer: Urn<->String).
- Protobuf symbol table: 6673 entries, hash 1420265035, advertised as zephyr-6673. Tags 0x02/0x03/0x0F/0x14 for strings. Local symbol tables (0x0E) and included records (0x0C) for deduplication.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Null vs absent field behavior documented
- [x] #2 Timestamp format confirmed (seconds vs milliseconds)
- [x] #3 Nested union discriminator format documented
- [x] #4 Findings written to re/serialization_edge_cases.md
<!-- AC:END -->

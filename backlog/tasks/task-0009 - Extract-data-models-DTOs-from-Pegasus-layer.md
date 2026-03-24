---
id: TASK-0009
title: Extract data models/DTOs from Pegasus layer
status: Done
assignee:
  - '@claude'
created_date: '2026-03-23 23:00'
updated_date: '2026-03-24 05:48'
labels:
  - phase3
  - static-analysis
  - models
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Analyze the ~1458 Pegasus-generated model classes under com.linkedin.android.pegasus.gen.voyager to extract the data schema for each domain. Map field names, types, optional/required status, and URN references. Focus on the models most relevant to our in-scope features: Me, MiniProfile, Profile, ProfileView, Update/UpdateV2, Conversation, Event, MessageEvent, JobPosting, Connection, Invitation, Notification. This will directly inform the Rust struct definitions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Key model schemas documented in re/data_models.md
- [x] #2 URN format and entity reference patterns documented
- [x] #3 Collection/pagination metadata model documented
- [x] #4 At least 10 core models fully mapped (fields, types, optionality)
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Cataloged all sub-packages under voyager (20 top-level, 100+ sub-packages)
- Read and documented 15+ core model classes across all priority domains
- Identified base interfaces: DataTemplate -> RecordTemplate (structs), UnionTemplate (enums)
- Mapped JSON field names from accept() method DataProcessor calls
- Documented optionality pattern (hasXxx boolean flags + builder validation)
- Cataloged 12 enum types critical for Rust serde
- Mapped union types requiring custom deserialization (Image, EventContent, MessagingProfile, Invitee)
- Created priority tiers for Rust implementation order
- Wrote comprehensive re/pegasus_models.md with full field tables
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Extracted and documented Pegasus data models from the decompiled LinkedIn Android app.

Output: re/pegasus_models.md (comprehensive reference for Rust library implementation)

Key findings:
- All Pegasus models implement RecordTemplate (structs) or UnionTemplate (tagged unions) via DataTemplate base interface
- JSON field names are embedded in accept() method calls to DataProcessor.startRecordField()
- Optionality is tracked via hasXxx boolean flags; required fields are validated in build(Flavor.RECORD)
- All enums include $UNKNOWN catch-all variant for forward compatibility
- URNs (urn:li:{type}:{id}) are the universal entity reference format
- Union types use Rest.li FQN strings as discriminator keys in JSON

Models documented (15+ fully mapped with field tables):
- Common: Me, CollectionMetadata, Image (union), VectorImage, Date, DateRange, GraphDistance
- Identity: MiniProfile, Profile, Position, Education, PublicContactInfo
- Messaging: Conversation, Event, EventContent (union), MessageEvent, MessagingProfile (union), MessagingMember
- Feed: UpdateV2, SocialDetail, SocialActivityCounts, Comment
- Jobs: JobDescription, JobDetails + enums (JobState, ApplicationFlow, JobSeekerStatus)
- Connections: Connection, Invitation + InvitationType enum
- Search: SearchProfile
- Notifications: Card

Enums documented: ReactionType (11 variants), EventSubtype (16), MessageRequestState, NotificationStatus, ConversationAction, MailboxFolder, CommentAction, GraphDistance, InvitationType, JobState, ApplicationFlow, JobSeekerStatus

Includes Java-to-Rust type mapping table and 4-tier implementation priority guide.
<!-- SECTION:FINAL_SUMMARY:END -->

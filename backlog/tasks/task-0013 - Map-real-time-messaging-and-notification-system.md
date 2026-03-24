---
id: TASK-0013
title: Map real-time messaging and notification system
status: Done
assignee:
  - '@claude'
created_date: '2026-03-23 23:01'
updated_date: '2026-03-24 05:59'
labels:
  - phase3
  - static-analysis
  - realtime
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Investigate the real-time communication system used for messaging and notifications. Document the long-polling mechanism (LongPollStreamNetworkClient), subscription topic format (URN-based), real-time frontend endpoints (/realtime/realtimeFrontendSubscriptions, /realtime/realtimeFrontendTimestamp), connection lifecycle, reconnection logic, and how message events are delivered. This is essential for implementing real-time messaging in the Rust client.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Real-time subscription protocol documented
- [x] #2 Topic format and subscription management documented
- [x] #3 Message delivery event format documented
- [x] #4 Connection lifecycle and reconnection logic documented
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Traced the entire real-time system from decompiled jadx sources:
- com.linkedin.android.realtime.internal (core): RealTimeConnection, RealTimeOnlineManager, EventProcessor, SubscriptionManager, HttpUtils, BackoffComputer, ServerTimeManager, SystemSubscriptions
- com.linkedin.android.realtime.api: RealTimeManager, RealTimeConfig, SubscriptionInfo, Subscriber, ConnectionRetryStrategy, RealTimeUrnFactory
- com.linkedin.android.realtime.realtimefrontend: RealtimeEvent, DecoratedEvent, BatchSubscriptionRequest/Status
- com.linkedin.android.networking: LongPollStreamNetworkClient, LongPollStreamResponseHandler
- com.linkedin.android.messaging.integration.realtime: EventSubscriptionInfo, ConversationSubscriptionInfo, TypingIndicatorSubscriptionInfo, SeenReceiptSubscriptionInfo, SmartRepliesSubscriptionInfoV2
- com.linkedin.android.l2m.badge: BadgerSubscriptionInfo
- com.linkedin.android.infra.ui.messaging.presence: PresenceStatusManager

Key findings:
- Transport is SSE (text/event-stream), NOT WebSocket or MQTT
- Endpoint: GET /realtime/connect (not under /voyager/api/ prefix)
- Same cookie auth as regular API, no special tokens
- JSON payloads (Pegasus models), no protobuf
- 7 known topic types documented
- Full lifecycle, reconnection, and retry logic mapped
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Mapped LinkedIn Android real-time messaging and notification system from decompiled sources. Created re/realtime_system.md documenting:

**Transport**: Server-Sent Events (SSE) over HTTP long poll at GET /realtime/connect. No WebSocket, MQTT, or custom protocol.

**Authentication**: Same cookie-based auth (li_at, JSESSIONID/Csrf-Token) as regular API. Additional headers: Accept: text/event-stream, x-li-realtime-session (UUID).

**Message format**: JSON (Pegasus models). SSE stream delivers a RealtimeEvent union: ClientConnection (handshake), Heartbeat (keepalive), or DecoratedEvent (topic + typed payload).

**7 event topics identified**: messagesTopic (new messages), conversationsTopic (conversation metadata), typingIndicatorsTopic, messageSeenReceiptsTopic, replySuggestionTopicV2 (smart replies), tabBadgeUpdateTopic (badge counts), presenceStatusTopic (online status).

**Subscription protocol**: Rest.li BATCH_UPDATE/BATCH_DELETE to /realtime/realtimeFrontendSubscriptions with compound keys (topic URN + connection ID).

**Reconnection**: LinearBackoffStrategy with max 2 retries (5-14s delays). Connection only active in foreground. Server time sync via 4-request NTP-like protocol. HTTP 412 triggers full reconnect.
<!-- SECTION:FINAL_SUMMARY:END -->

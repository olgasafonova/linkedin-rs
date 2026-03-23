# AndroidManifest.xml Analysis

**Package**: `com.linkedin.android`
**Target SDK**: 29 (Android 10)
**Install Location**: internalOnly
**Application Class**: `com.linkedin.android.app.FlagshipApplication`

---

## Application-Level Config

- `allowBackup="false"` -- no auto-backup
- `largeHeap="true"` -- requests large Dalvik heap
- `usesCleartextTraffic="true"` -- allows HTTP (not just HTTPS)
- `networkSecurityConfig="@xml/p"` -- cleartext permitted globally; debug builds trust user-installed CAs (useful for MITM interception in debug builds)
- `supportsRtl="true"` -- right-to-left layout support
- Uses `org.apache.http.legacy` library (optional)

---

## Permissions

### Standard Android Permissions

| Permission | Purpose |
|---|---|
| `INTERNET` | Network access |
| `READ_EXTERNAL_STORAGE` / `WRITE_EXTERNAL_STORAGE` | File access |
| `READ_CALENDAR` | Calendar sync for events |
| `CAMERA` | Profile photos, scanning |
| `RECORD_AUDIO` (SDK 23+) | Voice messages, video calls |
| `ACCESS_NETWORK_STATE` / `CHANGE_NETWORK_STATE` | Network monitoring |
| `ACCESS_WIFI_STATE` / `CHANGE_WIFI_STATE` | WiFi detection |
| `VIBRATE` | Notification haptics |
| `READ_PHONE_STATE` | Device identity |
| `WAKE_LOCK` | Background processing |
| `RECEIVE_BOOT_COMPLETED` | Start on boot |
| `FOREGROUND_SERVICE` | Long-running tasks |
| `GET_TASKS` | Task stack info |
| `NFC` (SDK 23+) | Near-field communication (profile sharing) |
| `READ_SYNC_SETTINGS` / `WRITE_SYNC_SETTINGS` | Contact sync |
| `AUTHENTICATE_ACCOUNTS` / `MANAGE_ACCOUNTS` | Account management |
| `REQUEST_INSTALL_PACKAGES` | APK install (updates?) |
| `WRITE_SETTINGS` | System settings |
| `READ_APP_BADGE` | Badge count |

### Custom Permissions (Defined by LinkedIn)

| Permission | Protection Level |
|---|---|
| `com.linkedin.android.permission.C2D_MESSAGE` | signature |
| `com.linkedin.android.permission.MIPUSH_RECEIVE` | signature |
| `com.linkedin.android.permission.PROCESS_PUSH_MSG` | signatureOrSystem |
| `com.linkedin.android.permission.PUSH_PROVIDER` | signatureOrSystem |
| `com.linkedin.android.permission.PUSH_WRITE_PROVIDER` | signatureOrSystem |
| `getui.permission.GetuiService.com.linkedin.android` | normal |

### Third-Party Push Permissions

- `com.google.android.c2dm.permission.RECEIVE` -- GCM/FCM
- `com.vivo.notification.permission.BADGE_ICON` -- Vivo badges
- `com.coloros.mcs.permission.RECIEVE_MCS_MESSAGE` -- Oppo/ColorOS push
- `com.heytap.mcs.permission.RECIEVE_MCS_MESSAGE` -- Oppo/HeyTap push
- `com.asus.msa.SupplementaryDID.ACCESS` -- Asus device ID
- `freemme.permission.msa` -- Chinese OEM device ID
- Various Huawei, Samsung, HTC, Sony launcher badge permissions (SDK 23+)

### Billing

- `com.android.vending.BILLING` -- Google Play in-app billing (Premium subscriptions)
- `com.google.android.finsky.permission.BIND_GET_INSTALL_REFERRER_SERVICE` -- Install attribution

---

## Key Activities

### Launch / Auth Flow

| Activity | Role |
|---|---|
| `authenticator.LaunchActivity` | **MAIN LAUNCHER**. Also handles third-party login (`AUTHORIZE_APP_LOGIN`) and NFC discovery (`application/vnd.com.linkedin.android`) |
| `authenticator.RealLaunchActivity` | Actual launch after splash |
| `growth.login.LoginActivity` | Login screen |
| `growth.onboarding.OnboardingActivity` | Post-signup onboarding |
| `growth.onboarding.SingleStepOnboardingActivity` | Simplified onboarding |
| `liauthlib.thirdparty.LiThirdPartyAuthorizeActivity` | **Third-party OAuth authorization** (action: `AUTHORIZE_APP`) |
| `liauthlib.thirdparty.LiThirdPartyViewUrlActivity` | OAuth URL viewer |
| `liauthlib.thirdparty.LiThirdPartyWebViewAuthorizeActivity` | OAuth in WebView |
| `liauthlib.LiAuthWebActivity` | Auth web view |
| `liauthlib.biometric.DeviceCredentialVerificationActivity` | Biometric/PIN verification |
| `liauthlib.registration.ChallengeWebViewActivity` | CAPTCHA/challenge handling |
| `liauthlib.registration.ChallengeWebViewV2Activity` | Challenge v2 |
| `lioneclicklogin.LiOneClickLoginInitActivity` | One-click login (theme: NoDisplay) |
| `developer.OAuthTokenHelperActivity` | OAuth token helper |

### Core Navigation

| Activity | Role |
|---|---|
| `home.HomeActivity` | Main home/feed screen (`alwaysRetainTaskState`) |
| `search.SearchActivity` | Global search |
| `settings.ui.SettingsActivity` | Settings |
| `premium.PremiumActivity` | Premium upsell |

### Profile

| Activity | Role |
|---|---|
| `identity.profile.shared.view.ProfileViewActivity` | View any profile |
| `identity.profile.shared.edit.ProfileEditHostActivity` | Edit profile |
| `identity.profile.self.guidededit.infra.GuidedEditActivity` | Guided profile editing |
| `identity.zephyrguidededit.GuidedEditV2Activity` | Guided edit v2 |
| `identity.profile.reputation.view.recommendations.ProfileRecommendationActivity` | Recommendations |
| `identity.profile.reputation.edit.skills.ProfileSkillsEditHostActivity` | Skills editing |
| `identity.profile.reputation.view.saveditems.SavedItemsActivity` | Saved items |
| `identity.me.wvmp.WvmpActivity` | "Who viewed my profile" |
| `identity.me.wvmp.privatemode.WvmpPrivateModeActivity` | Private mode WVMP |
| `identity.me.shared.profilehost.MeProfileHostActivity` | Own profile host |
| `identity.profile.view.gamification.ProfileGamificationActivity` | Profile strength gamification |

### Feed / Publishing

| Activity | Role |
|---|---|
| `feed.conversation.updatedetail.FeedUpdateDetailActivity` | Post detail view |
| `feed.conversation.commentdetail.CommentDetailActivity` | Comment detail |
| `feed.conversation.likesdetail.LikesDetailActivity` | Likes list |
| `feed.conversation.socialdrawer.SocialDrawerActivity` | Social drawer (reactions) |
| `feed.page.imagegallery.FeedImageGalleryActivity` | Image gallery |
| `feed.page.aggregate.AggregateActivity` | Aggregated feed view |
| `feed.interest.contenttopic.FeedContentTopicActivity` | Content topics |
| `feed.interest.plaza.FeedInterestPlazaActivity` | Interest plaza |
| `feed.follow.preferences.followershub.FollowersHubActivity` | Followers hub |
| `publishing.sharing.ShareActivity` | Share/compose post |
| `publishing.sharing.compose.poll.PollComposeActivity` | Create poll |
| `publishing.sharing.compose.mention.MentionActivity` | @mention picker |
| `publishing.reader.ArticleActivity` | Article reader |
| `publishing.storyline.page.StorylineActivity` | Stories |
| `publishing.shared.videoviewer.VideoViewerActivity` | Video viewer |
| `publishing.document.DocumentActivity` | Document viewer |
| `publishing.contentanalytics.ContentAnalyticsActivity` | Content analytics |

### Messaging

| Activity | Role |
|---|---|
| `messaging.ZephyrMessagingHomeActivity` | Messaging home (portrait-only) |
| `messaging.messagelist.MessageListActivity` | Conversation view |
| `messaging.compose.ComposeActivity` | New message |
| `messaging.compose.InmailComposeActivity` | InMail compose |
| `messaging.inlinereply.InlineReplyActivity` | Notification inline reply |
| `messaging.participantdetails.ParticipantDetailsActivity` | Participant details |
| `messaging.participantdetails.AddParticipantActivity` | Add to group |
| `messaging.conversationlist.ConversationSearchListActivity` | Search conversations |

### Jobs

| Activity | Role |
|---|---|
| `entities.job.controllers.JobActivity` | Job detail |
| `jobs.JobsMainActivity` | Jobs home |
| `jobs.preference.JobsPreferenceActivity` | Job preferences |
| `jobs.manager.ZephyrJobsManagerDetailActivity` | Job manager (poster view) |
| `jobs.savedsearch.JobSavedSearchResultListActivity` | Saved job searches |
| `jobs.salary.SalaryActivity` | Salary insights |
| `jobs.categories.JobsCategoriesActivity` | Job categories |
| `entities.jymbii.JymbiiActivity` | "Jobs you may be interested in" |
| `entities.jobsearchalert.JobSearchAlertActivity` | Job alerts |
| `jobs.review.CompanyReviewViewAllActivity` | Company reviews |

### Network / Connections

| Activity | Role |
|---|---|
| `mynetwork.RelationshipsSecondaryActivity` | My Network secondary |
| `mynetwork.nymk.NymkActivity` | "People You May Know" |
| `mynetwork.scan.ScanActivity` | QR code scanner |
| `growth.abi.nearby_people_v2.NearbyV2Activity` | Nearby people (Bluetooth/NFC) |
| `entities.company.controllers.CompanyActivity` | Company page |

### Notifications

| Activity | Role |
|---|---|
| `identity.me.notifications.NotificationsActivity` | Notifications list |
| `identity.me.notifications.settings.NotificationSettingActivity` | Notification settings |
| `identity.me.notifications.contextualresponse.ContextualResponseActivity` | Quick response |
| `identity.me.notifications.AppreciationActivity` | Kudos/appreciation |

### Deep Linking

| Activity | Role |
|---|---|
| `l2m.deeplink.DeepLinkHelperActivity` | **Primary deep link handler** -- handles `linkedin://` scheme and all `*.linkedin.com` HTTP/HTTPS URLs |
| `l2m.deeplink.CustomDeepLinkHelperActivity` | Custom deep links (`linkedin://ads`) |
| `urls.DeeplinkActivity` | **Secondary deep link handler** -- handles all `*.linkedin.com` HTTP/HTTPS with extensive path patterns |
| `redirect.DeepLinkToWebViewerRedirectActivity` | Deep link to WebView redirect |
| `l2m.shortlink.ShortlinkResolveActivity` | Short link resolver |

### Other Notable

| Activity | Role |
|---|---|
| `learning.LearningActivity` | LinkedIn Learning |
| `wxapi.WXEntryActivity` | WeChat integration (`singleTask`, `exported=true`) |
| `growth.samsung.SamsungSyncConsentActivity` | Samsung contact sync consent |
| `tos.ui.ToSWebviewActivity` | Terms of Service |
| `shaky.FeedbackActivity` | Shake-to-report feedback |
| `infra.DevTeamActivity` | Dev team/debug screen |
| `dev.settings.OverlaySettingsActivity` | Dev overlay settings |
| `reactnative.ReactNativeDemoActivity` | React Native demo (uses RN) |
| `facebook.react.devsupport.DevSettingsActivity` | React Native dev settings |

---

## Services

### LinkedIn Services

| Service | Purpose |
|---|---|
| `developer.OAuthService` | **SSO token provider** (exported, action: `GET_TOKEN`, category: `SSO`) |
| `liauthlib.sso.LiSSOService` | **SSO service** (exported, actions: `GET_ACCOUNTS`, `GET_TOKENS`, category: `SSO`) |
| `growth.contactsync.AuthenticatorService` | Android account authenticator (exported, permission: `ACCOUNT_MANAGER`) |
| `growth.calendar.CalendarUploadService` | Calendar upload (BIND_JOB_SERVICE) |
| `messaging.messagelist.ConversationPrefetchJobService` | Message prefetch |
| `messaging.inlinereply.InlineReplyIntentService` | Inline reply handler |
| `publishing.shared.preprocessing.MediaPreprocessorService` | Media preprocessing |
| `publishing.shared.mediaupload.VectorService` | Media upload |
| `deeplink.services.DeferredDeeplinkingService` | Deferred deep linking (exported) |
| `webrouter.customtabs.impl.KeepAliveService` | Custom tabs keep-alive (exported) |
| `l2m.notification.GetuiPushIntentService` | GeTui push intent processing |
| `l2m.notification.NotificationListenerService` | Notification listener |
| `l2m.notification.RegistrationJobIntentService` | Push registration |
| `l2m.notification.DailyRundownNotificationsPushActionTrackingIntentService` | Daily rundown tracking |
| `l2m.guestnotification.GuestLocalNotificationService` | Guest notifications |
| `appwidget.newsmodule.NewsModuleService` | Widget news data (BIND_REMOTEVIEWS) |
| `dev.settings.OverlayService` | Dev overlay |

### Third-Party Push Services

| Service | Platform |
|---|---|
| `com.google.firebase.messaging.FirebaseMessagingService` | Firebase Cloud Messaging (Google) |
| `com.google.firebase.iid.FirebaseInstanceIdService` | Firebase Instance ID |
| `com.igexin.sdk.PushService` | GeTui Push (China) -- runs in `:pushservice` process |
| `com.igexin.sdk.HmsPushMessageService` | GeTui via Huawei HMS |
| `com.igexin.sdk.OppoPushService` | GeTui via Oppo/ColorOS |
| `com.igexin.sdk.OppoAppPushService` | GeTui via Oppo/HeyTap |
| `com.vivo.push.sdk.service.CommandClientService` | Vivo Push |
| `com.xiaomi.push.service.XMPushService` | Xiaomi MiPush |
| `com.xiaomi.push.service.XMJobService` | Xiaomi MiPush job |
| `com.xiaomi.mipush.sdk.PushMessageHandler` | Xiaomi message handler |
| `com.xiaomi.mipush.sdk.MessageHandleService` | Xiaomi message handle |
| `com.huawei.hms.support.api.push.service.HmsMsgService` | Huawei HMS Push |
| `com.huawei.agconnect.core.ServiceDiscovery` | Huawei AGConnect |
| `com.google.android.gms.measurement.AppMeasurementService` | Google Analytics |
| `com.google.android.gms.auth.api.signin.RevocationBoundService` | Google Sign-In revocation |

### AndroidX / Infra Services

- `androidx.work.impl.background.systemalarm.SystemAlarmService`
- `androidx.work.impl.background.systemjob.SystemJobService`
- `androidx.work.impl.foreground.SystemForegroundService`
- `androidx.room.MultiInstanceInvalidationService`

---

## Content Providers

| Provider | Authority | Exported | Purpose |
|---|---|---|---|
| `androidx.core.content.FileProvider` | `com.linkedin.android.fileprovider` | No | File sharing |
| `com.linkedin.android.realtime.internal.SystemSubscriptions` | `com.linkedin.android.RealTimeSystemSubscriptions` | No | **Real-time event subscriptions** |
| `androidx.lifecycle.ProcessLifecycleOwnerInitializer` | `com.linkedin.android.lifecycle-process` | No | Lifecycle init |
| `com.getui.gtc.base.GtcProvider` | `com.linkedin.android.getui.gtc.provider` | No | GeTui push init |
| `com.huawei.hms.support.api.push.PushProvider` | `com.linkedin.android.huawei.push.provider` | **Yes** | Huawei push (protected by signature permission) |
| `com.huawei.hms.aaid.InitProvider` | `com.linkedin.android.aaidinitprovider` | No | Huawei AAID init |
| `com.huawei.hms.update.provider.UpdateProvider` | `com.linkedin.android.hms.update.provider` | No | HMS update |
| `com.huawei.agconnect.core.provider.AGConnectInitializeProvider` | `com.linkedin.android.AGCInitializeProvider` | No | AGConnect init |
| `com.huawei.updatesdk.fileprovider.UpdateSdkFileProvider` | `com.linkedin.android.updateSdk.fileProvider` | No | HMS update files |

---

## Intent Filters / Deep Link Scheme Map

### Custom URI Scheme: `linkedin://`

| Host | Handler | Purpose |
|---|---|---|
| `ads` | `CustomDeepLinkHelperActivity` | Ads deep link |
| `you` | `DeepLinkHelperActivity` | User-related |
| `union` | `DeepLinkHelperActivity` | Union/connection |
| `profile` | `DeepLinkHelperActivity` | Profile view |
| `jobs` | `DeepLinkHelperActivity` | Jobs |
| `oneclicklogin` | `DeepLinkHelperActivity` | One-click login (x2 filters) |
| `contactsyncconsent` | `SamsungSyncConsentActivity` | Samsung sync consent |

### HTTP/HTTPS App Links (autoVerify=true)

Two activities handle `*.linkedin.com` URLs:

**`DeepLinkHelperActivity`** handles these URL paths on `www.linkedin.com` + 80+ country subdomains + `www.linkedin.cn` + `www.linkedinmobileapp.com` + `www.chitu.com` (Chinese partner):

Key paths (abbreviated, `/comm/` variants omitted -- every path has a `/comm/` prefix variant):

- `/login`, `/`, `/m`, `/m/`
- `/in/.*` -- profile by vanity URL
- `/company/.*`, `/school/.*` -- company/school pages
- `/jobs/.*`, `/job/.*`, `/jobs/view/.*`, `/jobs/search` -- job listings
- `/messaging`, `/messaging/thread/.*`, `/messaging/compose/.*` -- messaging
- `/mynetwork`, `/mynetwork/invite-accept/.*`, `/mynetwork/add-connections` -- network
- `/feed/.*`, `/feed/update/.*`, `/feed/share`, `/share` -- feed/posts
- `/me`, `/me/profile-views`, `/notifications` -- self/notifications
- `/settings`, `/settings/privacy`, `/settings/messages` -- settings
- `/premium/products` -- premium
- `/pulse/.*` -- articles
- `/hp`, `/nhome` -- home page
- `/profinder/.*` -- ProFinder
- `/search/results/content/` -- content search
- `/recs/give`, `/recs/received` -- recommendations
- `/pub/dir/.*` -- public directory
- `/groups/.*`, `/group/.*` -- groups
- `/psettings/email/confirm` -- email confirmation
- `/start/`, `/start/welcome`, `/start/boost-promo` -- onboarding
- `ptdrv.linkedin.com/*` -- PointDrive
- `view.pointdrive.linkedin.com/presentations/*` -- PointDrive presentations

**`DeeplinkActivity`** (secondary handler, also `autoVerify=true`) handles the same country domains plus `linkedin.com` (bare) and `linkedin.cn` (bare):

Additional paths it handles:
- `/in/.*/detail/interests` -- profile interests
- `/feed/update/.*/video`, `/feed/update/.*/video-with-related-content` -- video
- `/feed/topic/.*`, `/feed/hashtag/.*`, `/feed/news/.*` -- topics/hashtags
- `/feed/follow`, `/feed/followers`, `/feed/interest-panel` -- follow management
- `/feed/aggregated-share/.*`, `/feed/feed-campaign/.*` -- aggregated/campaign
- `/jobs/saved-jobs`, `/jobs/applied-jobs`, `/jobs/alerts`, `/jobs/career-interests` -- job management
- `/job/home`, `/job/applicant-management`, `/job/recruiter-home` -- job management
- `/messaging/inmail/compose/.*` -- InMail compose
- `/mynetwork/invite-connect/invitations`, `/mynetwork/connection-suggestions` -- network
- `/company/.*/campaign/.*`, `/organization/.*/campaign/.*` -- company campaigns
- `/career/home`, `/career/salaryInsight` -- career tools
- `/search/results/people`, `/search/results/companies`, `/search/results/schools`, `/search/results/index` -- search
- `/recommended/pymk`, `/recommended/follow`, `/people/pymk` -- people suggestions
- `/notifications/.*` -- notifications
- `/me/talent-match`, `/me/job-poster-badge` -- me features
- `/video/collection/.*` -- video collections
- `/talent/post-a-job`, `/talent/compliance/dashboard` -- talent/hiring
- `/appreciation/create/.*` -- appreciation/kudos
- `/birthday-collection/.*` -- birthday features
- `/leadGenForm/.*` -- lead gen
- `/chat/.*` -- chat
- `/answerlist/.*` -- Q&A
- `/reward/.*` -- rewards/gamification
- `/wukong-web/.*` -- Chinese market features (Q&A, salary, learning, career path, company reflection)
- `/wujing-frontend/.*` -- Chinese market features (PK detail, Nova)
- `/wechat/miniprogram` -- WeChat mini program

### MIME Type Handlers

- `vnd.android.cursor.item/vnd.com.linkedin.android.profile` -- contacts integration (profile)
- `vnd.android.cursor.item/vnd.com.linkedin.android.messaging` -- contacts integration (messaging)
- `text/plain`, `image/*` -- share intent receivers (SEND / SEND_MULTIPLE)
- `application/vnd.com.linkedin.android` -- NFC beam

---

## Meta-Data / API Keys

### Google APIs

| Key | Value |
|---|---|
| `com.google.android.awareness.API_KEY` | `AIzaSyBiKpK_ihSa6h0pZpIdvnCitqhmRU4PkSc` |
| `com.google.android.nearby.messages.API_KEY` | `AIzaSyAf0dUr5dFpUcUkCsuE8ZRGQvxFqne-2u0` |
| `com.google.android.gms.version` | `@integer/cq` (resolved at build time) |
| `com.google.android.play.billingclient.version` | `3.0.3` |

### Push Notification IDs

| Platform | Key | Value |
|---|---|---|
| GeTui | `GETUI_APPID` | `OtwrLClimtAVAyVSsW3Qk8` |
| Huawei HMS | `com.huawei.hms.client.appid` | `appid=10224214` |
| Oppo | `OPPOPUSH_APPKEY` | `OP_9NRMxdj57I0w08c04088s4wkc` |
| Oppo | `OPPOPUSH_APPSECRET` | `OP_Dfa4f1690176B744F078D7F0A1144637` |
| Vivo | `com.vivo.push.api_key` | `ca7076e5-9bf5-43cd-81cd-408437294931` |
| Vivo | `com.vivo.push.app_id` | `14557` |
| Xiaomi | `MIPUSH_APPID` | `XM_2882303761517136280` |
| Xiaomi | `MIPUSH_APPKEY` | `XM_5911713661280` |

### Firebase

- `com.google.firebase.components:...AnalyticsConnectorRegistrar`
- `com.google.firebase.components:...iid.Registrar`

### Huawei HMS Versions

- `awareness:1.0.8.300`
- `push:5.0.2.300`
- `opendevice:5.0.2.300`
- `base:5.0.0.301`

---

## Receivers (Broadcast Receivers)

| Receiver | Trigger | Purpose |
|---|---|---|
| `l2m.InstallReferrerReceiver` | `INSTALL_REFERRER` | Install attribution |
| `infra.LocaleChangeReceiver` | `LOCALE_CHANGED` | Language change |
| `messaging.MessagingNotificationReceiver` | `messengerlib.SYNC_INTENT` | Messaging sync (disabled by default) |
| `l2m.notification.DismissNotificationReceiver` | Dismiss notification action | Clear notifications |
| `l2m.notification.ImmediatelyPushNotificationReceiver` | `BOOT_COMPLETED`, push actions | Immediate push on boot |
| `l2m.badge.BadgePeriodicFetchReceiver` | `BOOT_COMPLETED`, badge wake | Badge count fetch |
| `growth.calendar.CalendarUploadReceiver` | `BOOT_COMPLETED`, task wake | Calendar sync on boot |
| `l2m.badge.HuaweiBarrierReceiver` | Huawei awareness barrier | Huawei-specific badge |
| `l2m.notification.TimeFenceReceiver` | Time fence events | Scheduled notifications |
| `l2m.seed.PreInstallReceiver` | Pre-install broadcast | Pre-install detection |
| `l2m.notification.PackageReplacedReceiver` | `MY_PACKAGE_REPLACED` | Post-update actions |
| `appwidget.ResponsiveWidget` | `APPWIDGET_UPDATE` | App widget |
| `appwidget.ResponsiveWidget$ActiveUserListener` | `USER_PRESENT` | Widget active user (disabled) |
| `com.google.firebase.iid.FirebaseInstanceIdReceiver` | `c2dm.intent.RECEIVE` | FCM token |
| Various `androidx.work` constraint proxies | Battery, storage, network | WorkManager scheduling |
| Various Chinese push receivers | Platform-specific | Push notification routing |

---

## Key Findings for Reverse Engineering

### Auth Architecture

1. **Three auth entry points**:
   - `LaunchActivity` -- main app launch, also third-party login
   - `LiThirdPartyAuthorizeActivity` -- third-party OAuth (`com.linkedin.android.auth.AUTHORIZE_APP`)
   - `LiOneClickLoginInitActivity` -- one-click login (invisible activity)

2. **SSO infrastructure**:
   - `OAuthService` -- exported service providing tokens (action: `GET_TOKEN`, category: `SSO`)
   - `LiSSOService` -- exported SSO service (actions: `GET_ACCOUNTS`, `GET_TOKENS`)
   - Both are exported, meaning other apps can request LinkedIn auth tokens

3. **Auth library**: `com.linkedin.android.liauthlib` package contains:
   - `thirdparty.LiThirdPartyAuthorizeActivity` / `LiThirdPartyViewUrlActivity` / `LiThirdPartyWebViewAuthorizeActivity`
   - `biometric.DeviceCredentialVerificationActivity`
   - `registration.ChallengeWebViewActivity` / `ChallengeWebViewV2Activity`
   - `LiAuthWebActivity`
   - `common.DummyFragmentActivity`
   - `sso.LiSSOService`

4. **Account authenticator**: `AuthenticatorService` with `android.accounts.AccountAuthenticator` -- integrates with Android's account system for contact sync

### Real-Time / Messaging

1. **Real-time subscriptions**: `SystemSubscriptions` content provider at `com.linkedin.android.RealTimeSystemSubscriptions` -- likely manages WebSocket/SSE subscriptions
2. **Messaging sync**: `MessagingNotificationReceiver` listens for `com.linkedin.messengerlib.SYNC_INTENT` -- disabled by default, enabled at runtime
3. **Conversation prefetch**: `ConversationPrefetchJobService` -- background message prefetching
4. **Inline reply**: Direct reply from notifications via `InlineReplyIntentService`

### Push Notification Strategy

LinkedIn uses a multi-provider push strategy for maximum reach:
- **Google FCM** -- primary for most devices
- **GeTui (igexin)** -- Chinese push aggregator, acts as fallback and routes to:
  - Huawei HMS Push
  - Oppo/ColorOS Push
  - Vivo Push
  - Xiaomi MiPush
- All Chinese push services run in a separate `:pushservice` process

### Deep Link Architecture

- Two-layer deep link handling: `DeepLinkHelperActivity` (primary, handles `linkedin://` scheme + web URLs) and `DeeplinkActivity` (secondary, more path patterns)
- Both use `autoVerify=true` for Android App Links
- Extensive country-specific subdomain support (80+ countries)
- `/comm/` prefix variant for every path (likely "communication" tracking prefix)
- Chinese market has dedicated paths under `/wukong-web/` and `/wujing-frontend/`
- `DeferredDeeplinkingService` handles deferred deep links (install -> open -> navigate)

### Network Security

- **Cleartext traffic is allowed** globally (`usesCleartextTraffic="true"` + network security config)
- Debug builds trust user-installed certificates (enables MITM proxying)
- This is helpful for our reverse engineering -- we can intercept traffic

### Notable Packages (for Static Analysis)

Based on activity/service names, key packages to investigate in decompiled code:
- `com.linkedin.android.liauthlib` -- auth library
- `com.linkedin.android.authenticator` -- app authenticator
- `com.linkedin.android.lioneclicklogin` -- one-click login
- `com.linkedin.android.realtime.internal` -- real-time subscriptions
- `com.linkedin.android.messaging` -- messaging/conversations
- `com.linkedin.android.l2m` -- link-to-mobile (deep links, notifications, badges)
- `com.linkedin.android.urls` -- URL/deep link routing
- `com.linkedin.android.deeplink` -- deferred deep linking
- `com.linkedin.android.infra` -- infrastructure (WebView, networking)
- `com.linkedin.android.webrouter` -- web routing, custom tabs
- `com.linkedin.android.developer` -- OAuth/SSO for third-party devs

### React Native

The app includes React Native (`com.facebook.react.devsupport.DevSettingsActivity` and `ReactNativeDemoActivity`), suggesting some features may be built with RN. This is worth noting but likely not critical for API reverse engineering.

### Chinese Market (Chitu/Wukong)

Heavy investment in Chinese market with:
- `www.chitu.com` deep link support
- `www.linkedin.cn` support
- `/wukong-web/` paths for Q&A, salary, learning, career path, company reflection
- `/wujing-frontend/` paths for competitions/gamification
- WeChat mini program integration
- Shanyan SDK (`CmccLoginActivity`) for Chinese carrier-based login

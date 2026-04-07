# My Posts (Profile Updates V2) Endpoint

## Endpoint

```
GET /voyager/api/identity/profileUpdatesV2?q=memberShareFeed&profileUrn={encodedUrn}&moduleKey=member-shares%3Aphone&start={start}&count={count}
```

**Auth**: Requires `li_at` + `JSESSIONID` (standard session cookies).
**Premium**: Not required.

## Request

### Query Parameters

| Parameter | Required | Description |
|-----------|----------|-------------|
| `q` | Yes | Must be `memberShareFeed` |
| `profileUrn` | Yes | Rest.li-encoded `fsd_profile` URN of the target user |
| `moduleKey` | Yes | `member-shares:phone` (URL-encoded as `member-shares%3Aphone`) |
| `start` | No | 0-based pagination offset (default: 0) |
| `count` | No | Page size (default: 10) |

### Example

```
GET https://www.linkedin.com/voyager/api/identity/profileUpdatesV2?q=memberShareFeed&profileUrn=urn%253Ali%253Afsd_profile%253AACoAAABLusYB-ehcHySYcIqFeM1YX7qDSlcRsL8&moduleKey=member-shares%3Aphone&start=0&count=5
Csrf-Token: ajax:...
Cookie: li_at=...; JSESSIONID="ajax:..."
```

Note: The profile URN is double-encoded because Rest.li AsciiHex encoding encodes colons as `%3A`, and these `%` characters are then percent-encoded for the URL query string, resulting in `%253A`.

## Response Structure

Standard Rest.li collection response with `UpdateV2` elements -- same format as the general feed (`feed/updates?q=findFeed`).

```json
{
  "elements": [
    {
      "entityUrn": "urn:li:fs_updateV2:(urn:li:activity:7447168805107032064,MEMBER_SHARES,DEBUG_REASON,DEFAULT,false)",
      "value": {
        "com.linkedin.voyager.feed.render.UpdateV2": {
          "actor": { "name": { "text": "Olga Safonova" }, ... },
          "commentary": { "text": { "text": "Post content here..." } },
          "socialDetail": {
            "totalSocialActivityCounts": {
              "numLikes": 38,
              "numComments": 11,
              "numShares": 1,
              "numViews": 1515
            }
          }
        }
      }
    }
  ],
  "paging": {
    "count": 5,
    "start": 0,
    "links": []
  }
}
```

## Key Observations

1. **`numViews` is populated** for own posts, unlike in the general feed where it's often 0.
2. **Reshared posts appear** with the original author in the actor field, not the resharing user.
3. **The entityUrn** is a composite `fs_updateV2` URN containing the activity URN and context (`MEMBER_SHARES`).
4. **Same UpdateV2 format** as the general feed, so existing feed item parsing code works.
5. **No total count** in paging -- the API doesn't disclose total post count.

## Discovery Notes

- The legacy analytics endpoints (`identity/socialUpdateAnalyticsHeader`, `identity/socialUpdateAnalytics`) return HTTP 400 -- they likely require decoration/recipe IDs not documented in the static analysis or have been deprecated.
- This endpoint was discovered by probing `identity/profileUpdatesV2` with the `memberShareFeed` finder, which mirrors the pattern used by the LinkedIn mobile app's profile activity tab.
- Validated via live API testing (April 2026).

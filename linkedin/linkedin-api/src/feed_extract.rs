//! Extractors for Voyager feed `content` payloads.
//!
//! Feed responses carry articles, images, videos, documents, polls, and
//! carousels under a `content` object whose shape varies by post type. This
//! module hides the per-component walking behind a small set of pure
//! extractors so callers don't reach into the JSON themselves.

use serde_json::Value;

/// Article fields surfaced from a feed item that links to one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArticleInfo {
    pub title: String,
    pub url: String,
}

/// Extract article title + URL from a feed update's content. Returns `None`
/// when the item isn't an article (no articleComponent and no navigation
/// context).
pub fn extract_article_info(update: &Value) -> Option<ArticleInfo> {
    let content = update.get("content")?;
    extract_article_from_component(content).or_else(|| extract_article_from_nav(content))
}

fn extract_article_from_component(content: &Value) -> Option<ArticleInfo> {
    let article = content
        .get("articleComponent")
        .or_else(|| content.get("com.linkedin.voyager.feed.render.ArticleComponent"))?;
    let title = article
        .get("title")
        .and_then(|t| {
            t.get("text")
                .and_then(|v| v.as_str())
                .or_else(|| t.as_str())
        })
        .unwrap_or("")
        .to_string();
    let url = article
        .get("navigationContext")
        .and_then(|n| n.get("actionTarget"))
        .and_then(|v| v.as_str())
        .or_else(|| article.get("url").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();
    if title.is_empty() && url.is_empty() {
        return None;
    }
    Some(ArticleInfo { title, url })
}

fn extract_article_from_nav(content: &Value) -> Option<ArticleInfo> {
    let nav = content.get("navigationContext")?;
    let url = field_str(nav, "actionTarget").to_string();
    if url.is_empty() {
        return None;
    }
    let title = field_str(nav, "accessibilityText").to_string();
    Some(ArticleInfo { title, url })
}

fn field_str<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(|v| v.as_str()).unwrap_or("")
}

/// Component union keys that map directly to a media-type label.
const MEDIA_COMPONENT_LABELS: &[(&str, &str)] = &[
    ("com.linkedin.voyager.feed.render.ImageComponent", "image"),
    (
        "com.linkedin.voyager.feed.render.LinkedInVideoComponent",
        "video",
    ),
    (
        "com.linkedin.voyager.feed.render.DocumentComponent",
        "document",
    ),
    ("com.linkedin.voyager.feed.render.PollComponent", "poll"),
    (
        "com.linkedin.voyager.feed.render.ArticleComponent",
        "article",
    ),
    (
        "com.linkedin.voyager.feed.render.CelebrationComponent",
        "celebration",
    ),
    (
        "com.linkedin.voyager.feed.render.CarouselComponent",
        "carousel",
    ),
    ("imageComponent", "image"),
    ("videoComponent", "video"),
    ("documentComponent", "document"),
    ("pollComponent", "poll"),
    ("articleComponent", "article"),
    ("celebrationComponent", "celebration"),
    ("carouselComponent", "carousel"),
];

/// Substring tokens used as a fallback when the response only carries
/// `$type` rather than a typed component.
const MEDIA_TYPE_TOKENS: &[(&str, &str)] = &[
    ("Image", "image"),
    ("Video", "video"),
    ("Document", "document"),
    ("Poll", "poll"),
    ("Article", "article"),
];

/// Classify a feed update's media type. Empty string when no component or
/// `$type` token matches.
pub fn extract_media_type_label(update: &Value) -> String {
    let Some(content) = update.get("content") else {
        return String::new();
    };
    label_from_components(content)
        .or_else(|| label_from_type_token(content))
        .unwrap_or_default()
}

fn label_from_components(content: &Value) -> Option<String> {
    MEDIA_COMPONENT_LABELS
        .iter()
        .find(|(key, _)| content.get(*key).is_some())
        .map(|(_, label)| (*label).to_string())
}

fn label_from_type_token(content: &Value) -> Option<String> {
    let type_str = content.get("$type").and_then(|t| t.as_str())?;
    MEDIA_TYPE_TOKENS
        .iter()
        .find(|(token, _)| type_str.contains(token))
        .map(|(_, label)| (*label).to_string())
}

/// Extract media URLs (images, videos, documents, carousels) from a feed
/// item's content.
pub fn extract_media_urls(update: &Value) -> Vec<String> {
    let mut urls = Vec::new();
    let Some(content) = update.get("content") else {
        return urls;
    };
    extract_image_urls_into(content, &mut urls);
    extract_video_urls_into(content, &mut urls);
    extract_document_urls_into(content, &mut urls);
    extract_carousel_urls_into(content, &mut urls);
    urls
}

fn extract_image_urls_into(content: &Value, urls: &mut Vec<String>) {
    let Some(img) = component(content, "imageComponent", "ImageComponent") else {
        return;
    };
    collect_image_urls(img, urls);
}

fn extract_video_urls_into(content: &Value, urls: &mut Vec<String>) {
    let Some(vid) = component(content, "videoComponent", "LinkedInVideoComponent") else {
        return;
    };
    if let Some(url) = first_video_stream_url(vid) {
        urls.push(url);
    }
    if let Some(media) = vid
        .get("videoPlayMetadata")
        .or_else(|| vid.get("videoPlay"))
        .and_then(|m| m.get("media"))
        .and_then(|v| v.as_str())
    {
        urls.push(media.to_string());
    }
    if urls.is_empty() {
        if let Some(thumb) = video_thumbnail_url(vid) {
            urls.push(format!("(thumbnail) {}", thumb));
        }
    }
}

fn first_video_stream_url(vid: &Value) -> Option<String> {
    let play_meta = vid
        .get("videoPlayMetadata")
        .or_else(|| vid.get("videoPlay"))?;
    let streams = play_meta
        .get("progressiveStreams")
        .and_then(|s| s.as_array())?;
    streams.iter().find_map(|stream| {
        stream
            .get("streamingLocations")
            .and_then(|sl| sl.as_array())
            .and_then(|arr| arr.first())
            .and_then(|loc| loc.get("url"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
    })
}

fn video_thumbnail_url(vid: &Value) -> Option<&str> {
    let thumbnail = vid.get("thumbnail")?;
    thumbnail
        .get("url")
        .and_then(|v| v.as_str())
        .or_else(|| thumbnail.get("rootUrl").and_then(|v| v.as_str()))
}

fn extract_document_urls_into(content: &Value, urls: &mut Vec<String>) {
    let Some(doc) = component(content, "documentComponent", "DocumentComponent") else {
        return;
    };
    let url = doc.get("document").and_then(|d| {
        d.get("transcribedDocumentUrl")
            .and_then(|v| v.as_str())
            .or_else(|| d.get("downloadUrl").and_then(|v| v.as_str()))
    });
    if let Some(u) = url {
        urls.push(u.to_string());
    }
}

fn extract_carousel_urls_into(content: &Value, urls: &mut Vec<String>) {
    let Some(carousel) = component(content, "carouselComponent", "CarouselComponent") else {
        return;
    };
    let Some(pages) = carousel.get("pages").and_then(|p| p.as_array()) else {
        return;
    };
    for page in pages.iter().take(5) {
        if let Some(img) = page.get("imageComponent") {
            collect_image_urls(img, urls);
        }
    }
}

/// Look up a feed content component by its short name, falling back to the
/// fully-qualified Rest.li union key.
fn component<'a>(content: &'a Value, short: &str, type_name: &str) -> Option<&'a Value> {
    content
        .get(short)
        .or_else(|| content.get(format!("com.linkedin.voyager.feed.render.{}", type_name).as_str()))
}

fn collect_image_urls(img: &Value, urls: &mut Vec<String>) {
    if let Some(url) = first_image_attribute_url(img) {
        urls.push(url);
        return;
    }
    if let Some(url) = img.get("url").and_then(|v| v.as_str()) {
        urls.push(url.to_string());
    }
}

fn first_image_attribute_url(img: &Value) -> Option<String> {
    let images = img.get("images").and_then(|i| i.as_array())?;
    images
        .iter()
        .filter_map(|image| image.get("attributes").and_then(|a| a.as_array()))
        .flatten()
        .find_map(image_attribute_url)
}

fn image_attribute_url(attr: &Value) -> Option<String> {
    if let Some(url) = attr.get("imageUrl").and_then(|v| v.as_str()) {
        return Some(url.to_string());
    }
    let vi = attr.get("vectorImage")?;
    let root = vi.get("rootUrl").and_then(|v| v.as_str())?;
    let segment = vi
        .get("artifacts")
        .and_then(|a| a.as_array())
        .and_then(|arr| arr.last())
        .and_then(|a| {
            a.get("fileIdentifyingUrlPathSegment")
                .and_then(|v| v.as_str())
        })
        .unwrap_or("");
    Some(format!("{}{}", root, segment))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Assert that `update` extracts to exactly this title/url pair.
    fn assert_extracts_article(update: &serde_json::Value, title: &str, url: &str) {
        assert_eq!(
            extract_article_info(update),
            Some(ArticleInfo {
                title: title.to_string(),
                url: url.to_string(),
            })
        );
    }

    #[test]
    fn extract_article_info_reads_article_component() {
        let update = json!({
            "content": {
                "articleComponent": {
                    "title": {"text": "Headline"},
                    "navigationContext": {"actionTarget": "https://example.com/post"}
                }
            }
        });
        assert_extracts_article(&update, "Headline", "https://example.com/post");
    }

    #[test]
    fn extract_article_info_falls_back_to_nav_context() {
        let update = json!({
            "content": {
                "navigationContext": {
                    "actionTarget": "https://example.com/link",
                    "accessibilityText": "Open link"
                }
            }
        });
        assert_extracts_article(&update, "Open link", "https://example.com/link");
    }

    #[test]
    fn extract_article_info_none_when_neither() {
        let update = json!({"content": {"imageComponent": {}}});
        assert!(extract_article_info(&update).is_none());
    }

    #[test]
    fn extract_media_type_label_recognises_short_names() {
        let cases = [
            ("imageComponent", "image"),
            ("videoComponent", "video"),
            ("documentComponent", "document"),
            ("pollComponent", "poll"),
            ("articleComponent", "article"),
            ("carouselComponent", "carousel"),
            ("celebrationComponent", "celebration"),
        ];
        for (key, expected) in cases {
            let update = json!({"content": {key: {}}});
            assert_eq!(extract_media_type_label(&update), expected, "key={key}");
        }
    }

    #[test]
    fn extract_media_type_label_recognises_voyager_typenames() {
        let update = json!({
            "content": {
                "com.linkedin.voyager.feed.render.ImageComponent": {}
            }
        });
        assert_eq!(extract_media_type_label(&update), "image");
    }

    #[test]
    fn extract_media_type_label_falls_back_to_type_token() {
        let update = json!({
            "content": {
                "$type": "com.linkedin.voyager.feed.render.VideoSomething"
            }
        });
        assert_eq!(extract_media_type_label(&update), "video");
    }

    #[test]
    fn extract_media_type_label_empty_when_no_content() {
        let update = json!({});
        assert_eq!(extract_media_type_label(&update), "");
    }

    #[test]
    fn extract_media_urls_pulls_image_url() {
        let update = json!({
            "content": {
                "imageComponent": {
                    "images": [{
                        "attributes": [{"imageUrl": "https://media.example/i1.jpg"}]
                    }]
                }
            }
        });
        assert_eq!(
            extract_media_urls(&update),
            vec!["https://media.example/i1.jpg"]
        );
    }

    #[test]
    fn extract_media_urls_returns_empty_when_no_content() {
        assert_eq!(extract_media_urls(&json!({})), Vec::<String>::new());
    }
}

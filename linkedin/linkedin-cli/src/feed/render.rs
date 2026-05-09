//! Article + media-type extraction shared by `feed list` and `feed read`.

use serde_json::Value;

use super::helpers::field_str;

pub(super) struct ArticleInfo {
    pub title: String,
    pub url: String,
}

pub(super) fn extract_article_info(update: &Value) -> Option<ArticleInfo> {
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

pub(super) fn extract_media_type_label(update: &Value) -> String {
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

/// Extract media URLs (images, videos, documents) from a feed item's content.
pub(super) fn extract_media_urls(update: &Value) -> Vec<String> {
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

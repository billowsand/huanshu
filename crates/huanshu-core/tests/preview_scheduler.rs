//! `preview::PreviewCache` 的调度规则测试。
//!
//! 每条规则对应 `CLAUDE.md`「Typst 缩略图」与「编辑器实时预览」里的一段，对照表见汇报。
//! 测试里的「渲染器」是一个假的：它只按 `only` 生成带序号的图片，这样既能断言请求内容，
//! 也能统计每张图最后被交还了几次。

use std::collections::HashMap;

use huanshu_core::preview::{
    PreviewCache, PreviewOptions, PreviewRequest, PreviewResult, PreviewStyle, RenderedSlide,
};
use serde_json::{json, Value};

/// 测试用的图片：每次渲染都拿到一个新序号，正好用来断言「哪一张被替换掉了」。
type Img = u64;

fn style(width_px: f32) -> PreviewStyle {
    PreviewStyle {
        theme: Some("light-notes".to_string()),
        fonts: "{}".to_string(),
        width_px,
    }
}

fn cache() -> PreviewCache<Img> {
    PreviewCache::new(PreviewOptions::default())
}

fn deck(n: usize) -> Vec<Option<Value>> {
    (0..n)
        .map(|i| Some(json!({ "kind": "feature_grid", "title": format!("第 {i} 页") })))
        .collect()
}

/// 假的渲染器：给每一页一个新序号。
fn render(request: &PreviewRequest, cache: &mut PreviewCache<Img>) -> Vec<Img> {
    let mut released = cache.take_released();
    let result = PreviewResult::new(
        request
            .only
            .iter()
            .map(|position| RenderedSlide::new(*position, next_tag()))
            .collect(),
    );
    released.extend(cache.complete(request, Ok(result)));
    released
}

/// 全局递增的标签，模拟「每张新图都不一样」。
fn next_tag() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static TAG: AtomicU64 = AtomicU64::new(1);
    TAG.fetch_add(1, Ordering::Relaxed)
}

/// 把一个缓存跑到「所有想要的页都有图」为止，返回这一路交还的图片。
fn settle(cache: &mut PreviewCache<Img>) -> Vec<Img> {
    let mut released = Vec::new();
    while let Some(request) = cache.next_request() {
        released.extend(render(&request, cache));
    }
    released
}

#[test]
fn a_first_request_asks_for_every_page_of_the_deck() {
    let mut cache = cache();
    cache.set_deck(&deck(3), style(560.0));
    let request = cache.next_request().expect("nothing is cached yet");
    assert_eq!(
        request.slides.len(),
        3,
        "only slides with a blueprint are sent"
    );
    assert_eq!(request.only, vec![0, 1, 2]);
    assert_eq!(request.width_px, 576, "560 rounds up to the 64px bucket");
    let _ = render(&request, &mut cache);
}

#[test]
fn holes_in_the_deck_are_not_sent_and_pages_keep_their_own_indices() {
    let mut cache = cache();
    cache.set_deck(
        &[
            Some(json!({ "kind": "cover", "title": "封面" })),
            None, // 这一页还没生成
            Some(json!({ "kind": "closing", "title": "结束" })),
        ],
        style(560.0),
    );
    let request = cache.next_request().unwrap();
    assert_eq!(
        request.slides.len(),
        2,
        "the hole is not sent to the renderer"
    );
    assert_eq!(request.only, vec![0, 1], "positions are subset positions");

    // 渲染器按子集位置回报；缓存要把它映射回牌组下标。
    let _ = render(&request, &mut cache);
    assert!(cache.image(0).is_some());
    assert!(cache.image(1).is_none(), "deck index 1 is the hole");
    assert!(
        cache.image(2).is_some(),
        "deck index 2 is subset position 1"
    );
}

#[test]
fn nothing_is_requested_twice_while_the_shown_key_still_matches() {
    let mut cache = cache();
    cache.set_deck(&deck(2), style(560.0));
    settle(&mut cache);
    assert!(cache.next_request().is_none(), "everything is current");
}

/// 键 = 蓝图 + 位置 + 主题 + 字体 + 宽度。改主题后旧图继续显示，直到新图落下。
#[test]
fn a_theme_change_keeps_the_old_bitmap_on_screen_until_the_new_one_lands() {
    let mut cache = cache();
    cache.set_deck(&deck(1), style(560.0));
    settle(&mut cache);
    let old = *cache.image(0).expect("first render landed");

    let mut dark = style(560.0);
    dark.theme = Some("dark-blue".to_string());
    cache.set_deck(&deck(1), dark);

    let request = cache
        .next_request()
        .expect("the theme changed, so the key changed");
    assert_eq!(
        *cache.image(0).unwrap(),
        old,
        "the previous bitmap stays visible while the new one renders"
    );
    let released = render(&request, &mut cache);
    assert_eq!(released, vec![old], "the replaced bitmap is handed back");
    assert_ne!(*cache.image(0).unwrap(), old);
}

#[test]
fn a_width_change_rounds_into_the_same_bucket_and_does_not_recompile() {
    let mut cache = cache();
    cache.set_deck(&deck(1), style(560.0));
    settle(&mut cache);

    // 520 and 560 both round into 576: same key, no recompile.
    cache.set_deck(&deck(1), style(520.0));
    assert!(
        cache.next_request().is_none(),
        "both widths land in the same 576 bucket, so the cached key still matches"
    );

    // 600 rounds up to the next bucket, which does invalidate it.
    cache.set_deck(&deck(1), style(600.0));
    let request = cache
        .next_request()
        .expect("a new bucket invalidates the key");
    assert_eq!(request.width_px, 640);
    let _ = render(&request, &mut cache);
}

#[test]
fn only_the_pages_whose_key_changed_are_requested_again() {
    let mut cache = cache();
    let mut slides = deck(3);
    cache.set_deck(&slides, style(560.0));
    settle(&mut cache);

    slides[1].as_mut().unwrap()["title"] = json!("改过的标题");
    cache.set_deck(&slides, style(560.0));

    let request = cache.next_request().expect("page 1 changed");
    assert_eq!(request.only, vec![1], "the other two keys still match");
    let _ = render(&request, &mut cache);
}

#[test]
fn wanted_limits_the_request_but_not_the_deck_that_is_sent() {
    let mut cache = cache();
    cache.set_deck(&deck(10), style(560.0));
    cache.set_wanted(&[2, 5]);
    let request = cache.next_request().unwrap();
    assert_eq!(
        request.slides.len(),
        10,
        "cropping the deck by wanted would invalidate every page's key"
    );
    assert_eq!(request.only, vec![2, 5]);
    let _ = render(&request, &mut cache);

    // A page outside wanted is never asked for until it is wanted.
    assert!(cache.image(0).is_none());
    cache.set_wanted(&[0]);
    let request = cache.next_request().unwrap();
    assert_eq!(request.only, vec![0]);
}

#[test]
fn serial_merges_changes_during_a_request_into_one_follow_up() {
    let mut cache = cache();
    cache.set_deck(&deck(2), style(560.0));
    let first = cache.next_request().unwrap();

    // Two changes arrive while the first request is in flight.
    assert!(cache.next_request().is_none(), "serial: no second request");
    assert!(cache.next_request().is_none(), "still merged");
    assert!(cache.is_pending());

    let _ = render(&first, &mut cache);
    assert!(
        cache.next_request().is_none(),
        "nothing changed content, so the follow-up has nothing to ask for"
    );
}

/// `progressive`：过期但比屏幕上新的响应也先落下，键记成它自己的，于是最新内容仍会被再请求。
#[test]
fn progressive_lands_a_stale_but_newer_bitmap_and_still_asks_for_the_latest() {
    let mut cache = PreviewCache::new(PreviewOptions {
        progressive: true,
        ..PreviewOptions::default()
    });
    let mut slides = deck(1);
    cache.set_deck(&slides, style(560.0));
    let first = cache.next_request().unwrap();
    let _ = render(&first, &mut cache);
    let shown = *cache.image(0).unwrap();

    // 内容变了，发第二次请求，但在它回来之前再改一次（第三次）。
    slides[0].as_mut().unwrap()["title"] = json!("第二版");
    cache.set_deck(&slides, style(560.0));
    let second = cache.next_request().unwrap();
    slides[0].as_mut().unwrap()["title"] = json!("第三版");
    cache.set_deck(&slides, style(560.0));

    let mut released = render(&second, &mut cache);
    assert_ne!(
        *cache.image(0).unwrap(),
        shown,
        "the newer-but-stale bitmap lands immediately"
    );
    assert_eq!(released.len(), 1, "the bitmap it replaced is handed back");

    // 第二版的键与当前键不同，最新内容还会被再请求一次。
    let third = cache
        .next_request()
        .expect("the latest content is still wanted");
    assert_eq!(third.only, vec![0]);
}

#[test]
fn a_response_from_before_a_reset_is_discarded_whole() {
    let mut cache = cache();
    cache.set_deck(&deck(2), style(560.0));
    let request = cache.next_request().unwrap();

    // The user switched projects while the request was in flight.
    let released_by_reset = cache.reset();
    assert!(released_by_reset.is_empty(), "nothing was cached yet");
    let released = render(&request, &mut cache);
    assert_eq!(released.len(), 2, "both stale bitmaps are handed back");
    assert!(cache.image(0).is_none(), "a stale response lands nothing");
}

/// 换项目后那一页必须真的重新请求：旧的在途标记不能让画面一直空着。
#[test]
fn a_reset_makes_the_next_request_fire_even_with_an_old_pending_marker() {
    let mut cache = cache();
    cache.set_deck(&deck(1), style(560.0));
    let stale = cache.next_request().unwrap();
    cache.reset();

    cache.set_deck(&deck(1), style(560.0));
    // `serial`: the pre-reset request is still in flight, so nothing goes out yet.
    assert!(cache.next_request().is_none());
    let _ = render(&stale, &mut cache);
    assert!(
        cache.image(0).is_none(),
        "the pre-reset response lands nothing"
    );
    let fresh = cache
        .next_request()
        .expect("reset_pending forces a real request instead of trusting the stale marker");
    assert_eq!(fresh.only, vec![0]);
}

#[test]
fn a_failure_keeps_the_old_bitmap_and_does_not_retry_by_itself() {
    let mut cache = cache();
    cache.set_deck(&deck(1), style(560.0));
    settle(&mut cache);
    let shown = *cache.image(0).unwrap();

    let mut changed = deck(1);
    changed[0].as_mut().unwrap()["title"] = json!("编译不过");
    cache.set_deck(&changed, style(560.0));
    let request = cache.next_request().unwrap();
    let released = cache.complete(&request, Err("Typst 编译失败".to_string()));
    assert!(released.is_empty(), "a failure hands nothing back");
    assert_eq!(
        *cache.image(0).unwrap(),
        shown,
        "the old bitmap stays on screen behind the failure"
    );
    assert!(cache.failed());
    assert!(
        cache.next_request().is_none(),
        "no automatic retry: a deck that cannot compile would spin forever"
    );
    assert!(
        !cache.is_pending(),
        "and nothing is reported as still working"
    );

    // A content change re-arms it (the TypeScript version reacts to the same watchers).
    let mut edited = changed.clone();
    edited[0].as_mut().unwrap()["title"] = json!("又改了一次");
    cache.set_deck(&edited, style(560.0));
    let request = cache.next_request().expect("a new deck is requested again");
    let _ = cache.complete(&request, Err("又失败了".to_string()));
    assert!(cache.next_request().is_none(), "still no automatic retry");
    cache.refresh();
    assert!(
        cache.next_request().is_some(),
        "refresh() re-arms the request"
    );
}

#[test]
fn a_disabled_cache_does_not_request_but_keeps_its_bitmaps() {
    let mut cache: PreviewCache<Img> = PreviewCache::new(PreviewOptions {
        enabled: false,
        ..PreviewOptions::default()
    });
    cache.set_deck(&deck(1), style(560.0));
    assert!(cache.next_request().is_none());
    assert!(cache.image(0).is_none());
}

#[test]
fn max_cached_evicts_the_least_recently_wanted_and_asks_for_it_again_later() {
    let mut cache = PreviewCache::new(PreviewOptions {
        max_cached: Some(2),
        ..PreviewOptions::default()
    });
    cache.set_deck(&deck(4), style(560.0));
    cache.set_wanted(&[0, 1, 2, 3]);
    settle(&mut cache);
    assert_eq!(
        cache.cached(),
        4,
        "a page inside wanted is never evicted, however small max_cached is"
    );

    // 只想要前两页时，后两页才是淘汰候选。
    cache.set_wanted(&[0, 1]);
    let mut released = cache.take_released();
    assert!(
        cache.next_request().is_none(),
        "both wanted pages are current"
    );
    released.extend(cache.take_released());
    assert_eq!(cache.cached(), 2, "the cache is trimmed to max_cached");
    assert_eq!(released.len(), 2, "the evicted bitmaps are handed back");

    // 回到最早被点名的那一页：它必须被重新请求（否则键还相等，永远补不上图）。
    cache.set_wanted(&[3]);
    let request = cache
        .next_request()
        .expect("an evicted page is asked for again");
    assert_eq!(request.only, vec![3]);
}

#[test]
fn a_page_pending_keeps_its_placeholder_and_overflow_flags() {
    let mut cache = cache();
    cache.set_deck(&deck(1), style(560.0));
    let request = cache.next_request().unwrap();
    let result = PreviewResult::new(vec![RenderedSlide {
        index: 0,
        image: next_tag(),
        placeholder: true,
        overflow: true,
    }]);
    let _ = cache.complete(&request, Ok(result));
    assert!(cache.placeholder(0));
    assert!(cache.overflow(0));
}

#[test]
fn pages_that_leave_the_deck_release_their_bitmaps() {
    let mut cache = cache();
    cache.set_deck(&deck(3), style(560.0));
    settle(&mut cache);
    assert_eq!(cache.cached(), 3);

    cache.set_deck(&deck(1), style(560.0));
    let mut released = Vec::new();
    while let Some(request) = cache.next_request() {
        released.extend(render(&request, &mut cache));
    }
    released.extend(cache.take_released());
    assert_eq!(
        released.len(),
        2,
        "the two pages that vanished are handed back"
    );
    assert_eq!(cache.cached(), 1);
}

/// 每张进入过缓存的图，最后恰好被交还一次——GPUI 靠这个回收图集。
#[test]
fn every_bitmap_is_handed_back_exactly_once() {
    let mut cache = PreviewCache::new(PreviewOptions {
        max_cached: Some(2),
        ..PreviewOptions::default()
    });
    let mut released: Vec<Img> = Vec::new();
    let mut handed_in: Vec<Img> = Vec::new();

    let mut slides = deck(5);
    cache.set_deck(&slides, style(560.0));
    released.extend(cache.take_released());

    // First pass: fill the cache.
    while let Some(request) = cache.next_request() {
        released.extend(cache.take_released());
        let tags: Vec<Img> = request.only.iter().map(|_| next_tag()).collect();
        handed_in.extend(&tags);
        let result = PreviewResult::new(
            request
                .only
                .iter()
                .zip(tags)
                .map(|(position, tag)| RenderedSlide::new(*position, tag))
                .collect(),
        );
        released.extend(cache.complete(&request, Ok(result)));
        released.extend(cache.take_released());
    }

    // Then churn: edit every page twice, roam `wanted`, and finally reset.
    for round in 0..2 {
        for index in 0..5 {
            slides[index].as_mut().unwrap()["title"] = json!(format!("第 {round} 轮第 {index} 页"));
            cache.set_deck(&slides, style(560.0));
            cache.set_wanted(&[index, (index + 1) % 5]);
            while let Some(request) = cache.next_request() {
                released.extend(cache.take_released());
                let tags: Vec<Img> = request.only.iter().map(|_| next_tag()).collect();
                handed_in.extend(&tags);
                let result = PreviewResult::new(
                    request
                        .only
                        .iter()
                        .zip(tags)
                        .map(|(position, tag)| RenderedSlide::new(*position, tag))
                        .collect(),
                );
                released.extend(cache.complete(&request, Ok(result)));
                released.extend(cache.take_released());
            }
            released.extend(cache.take_released());
        }
    }
    released.extend(cache.reset());
    released.extend(cache.take_released());
    assert_eq!(cache.cached(), 0);

    let mut counted = released.clone();
    counted.sort_unstable();
    counted.dedup();
    assert_eq!(
        counted.len(),
        released.len(),
        "a bitmap was handed back twice: {counted:?} vs {released:?}"
    );
    assert_eq!(
        handed_in.len(),
        released.len(),
        "every bitmap that entered the cache is handed back: {} in, {} out",
        handed_in.len(),
        released.len()
    );
}

/// 一次请求回来后紧接着发下一次，直到无事可做——这是界面层的调用循环。
#[test]
fn a_failing_page_becomes_requestable_again_after_other_pages_arrive() {
    let mut cache = cache();
    let mut slides = deck(2);
    cache.set_deck(&slides, style(560.0));
    let first = cache.next_request().unwrap();
    let _ = cache.complete(&first, Err("boom".to_string()));
    assert!(cache.failed());

    // The user edits page 0; the retry carries page 0, page 1 is still pending.
    slides[0].as_mut().unwrap()["title"] = json!("改过");
    cache.set_deck(&slides, style(560.0));
    cache.refresh();
    let request = cache.next_request().expect("refresh re-arms the request");
    assert_eq!(request.only, vec![0, 1], "neither page ever got a bitmap");
    let _ = render(&request, &mut cache);
    assert!(!cache.failed());
}

/// 请求里带的宽度是档位后的值，界面层不必自己取整。
#[test]
fn bucket_width_always_returns_a_positive_multiple_of_the_bucket() {
    assert_eq!(PreviewCache::<Img>::bucket_width(0.0), 64);
    assert_eq!(PreviewCache::<Img>::bucket_width(-5.0), 64);
    assert_eq!(PreviewCache::<Img>::bucket_width(64.0), 64);
    assert_eq!(PreviewCache::<Img>::bucket_width(64.1), 128);
    assert_eq!(PreviewCache::<Img>::bucket_width(248.0 * 1.5), 384);
    for width in [1.0, 100.0, 999.0, 1664.0] {
        let bucket = PreviewCache::<Img>::bucket_width(width);
        assert!(bucket >= WIDTH_BUCKET_FLOOR && bucket % 64 == 0, "{bucket}");
    }
}

const WIDTH_BUCKET_FLOOR: u32 = 64;

/// 主题回落是渲染器给的，缓存如实传给界面层。
#[test]
fn the_theme_fallback_is_reported_back() {
    let mut cache = cache();
    cache.set_deck(&deck(1), style(560.0));
    let request = cache.next_request().unwrap();
    let mut result = PreviewResult::new(vec![RenderedSlide::new(0, next_tag())]);
    result.theme_fallback = Some("no-such-theme".to_string());
    let _ = cache.complete(&request, Ok(result));
    assert_eq!(cache.theme_fallback(), Some("no-such-theme"));
}

/// 演示模式：当前页还没有图时只请求当前页。
#[test]
fn a_single_wanted_page_produces_a_single_page_request() {
    let mut cache = PreviewCache::new(PreviewOptions {
        max_cached: Some(10),
        ..PreviewOptions::default()
    });
    cache.set_deck(&deck(29), style(1280.0));
    cache.set_wanted(&[7]);
    let request = cache.next_request().unwrap();
    assert_eq!(request.only, vec![7]);
    assert_eq!(request.slides.len(), 29);
    let _ = render(&request, &mut cache);

    // Prefetching the neighbours afterwards is just another `set_wanted`.
    cache.set_wanted(&[6, 7, 8, 9, 10]);
    let request = cache
        .next_request()
        .expect("the neighbours are not cached yet");
    assert_eq!(request.only, vec![6, 8, 9, 10], "page 7 is current already");
    let _ = render(&request, &mut cache);
    assert!(cache.next_request().is_none());
}

/// 收尾：把「键」的行为钉死——宽度变化必须真的改键，否则窗口缩放后画面不会更新。
#[test]
fn the_key_covers_theme_fonts_width_and_position() {
    let mut seen: HashMap<String, Vec<usize>> = HashMap::new();
    let mut slides = deck(2);

    for (theme, fonts, width) in [
        ("light-notes", "{}", 560.0),
        ("dark-blue", "{}", 560.0),
        ("light-notes", "{\"body\":\"A\"}", 560.0),
        ("light-notes", "{}", 640.0),
    ] {
        let mut cache = cache();
        cache.set_deck(
            &slides,
            PreviewStyle {
                theme: Some(theme.to_string()),
                fonts: fonts.to_string(),
                width_px: width,
            },
        );
        let request = cache.next_request().unwrap();
        seen.insert(format!("{theme}{fonts}{width}"), request.only.clone());
        let _ = render(&request, &mut cache);
        slides[0].as_mut().unwrap()["title"] = json!(format!("{theme}{fonts}{width}"));
    }
    for only in seen.values() {
        assert_eq!(
            *only,
            vec![0, 1],
            "page 0's key always changes, page 1's does not"
        );
    }
}

/// `reset` 只清缓存，不清牌组：清掉牌组会让这一次的请求永远为空，画面停在加载框。
#[test]
fn a_reset_keeps_the_deck_so_the_page_is_requested_again() {
    let mut cache = cache();
    cache.set_deck(&deck(2), style(560.0));
    settle(&mut cache);
    assert_eq!(cache.cached(), 2);

    let released = cache.reset();
    assert_eq!(released.len(), 2, "the cached bitmaps are handed back");
    assert_eq!(cache.cached(), 0);
    assert!(cache.image(0).is_none());

    // 不再 `set_deck`，直接就该重新请求。
    let request = cache
        .next_request()
        .expect("the deck survives the reset, so there is work to do");
    assert_eq!(request.slides.len(), 2);
    assert_eq!(request.only, vec![0, 1]);
}

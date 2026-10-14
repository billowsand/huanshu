//! 缩略图调度：**纯状态机**，不开线程、不碰渲染器。
//!
//! 规则照搬 `src/composables/useTypstThumbnails.ts`（Tauri 版仍在用同一套逻辑，两份并存到
//! Tauri 界面下线为止；**改一边必须改另一边**，`tests/preview_scheduler.rs` 逐条对着同一条规则）。
//! 界面层负责执行 [`PreviewCache::next_request`] 给出的请求，把结果交给
//! [`PreviewCache::complete`]，并把两处返回的图片逐张 `drop_image` 交还给 GPUI。
//!
//! 为什么图片要从返回值里拿回来：GPUI 不会回收图集里的 tile（见 spike 报告第 4 节第 2 条），
//! 宿主每丢一张图都必须自己 `drop_image`。因此**凡是进入过缓存的图，最后都会被交还恰好一次**
//! ——被替换、被淘汰、`reset` 清掉、或随牌组消失时都不例外。
//!
//! 时间相关的部分留在界面层：防抖由调用方排期；`serial` 把在途期间的变化合并成一次尾部请求，
//! 调用方只需在一次请求完成后再次调用 [`PreviewCache::next_request`]，直到返回 `None`。

use std::collections::{HashMap, HashSet};

use serde_json::Value;

/// 缩略图宽度向上取整到这个倍数（设备像素），避免窗口每变 1px 就整副重渲。
pub const WIDTH_BUCKET: u32 = 64;

/// 调度行为。默认与 Tauri 版一致（增量、按键落图）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviewOptions {
    /// 同一时刻最多一个请求在途，在途期间的变化合并成一次尾部请求。
    pub serial: bool,
    /// 过期但比当前显示的图新的响应也先落下（该页的键记成它自己的序号，最新内容仍会再被请求）。
    pub progressive: bool,
    /// 缓存里最多保留几张；超出时淘汰最久没被点名、且不在 `wanted` 里的。
    pub max_cached: Option<usize>,
    /// 为假时不发请求，已有的图保留。
    pub enabled: bool,
}

impl Default for PreviewOptions {
    fn default() -> Self {
        Self {
            serial: true,
            progressive: true,
            max_cached: None,
            enabled: true,
        }
    }
}

/// 键的一部分：主题、字体与宽度档位。任一变化都会让全部页的键改变。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PreviewStyle {
    pub theme: Option<String>,
    /// 字体设置的序列化形式（与 `FontSettings` 的 JSON 一致）。
    pub fonts: String,
    /// 显示宽度 × `window.scale_factor()`。
    pub width_px: f32,
}

/// 一次要发给渲染器的请求。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewRequest {
    /// 请求序号，`complete` 时原样带回；不是最新序号时响应整批作废。
    pub id: u64,
    /// 换目标（`reset`）的代数；请求记下发出时的值，回来时不相等就整批作废。
    pub generation: u64,
    /// 只含已有蓝图的子集；渲染器返回的 `index` 是**子集位置**。
    pub slides: Vec<Value>,
    pub theme: Option<String>,
    /// 已按 [`WIDTH_BUCKET`] 取整的设备像素宽度。
    pub width_px: u32,
    /// 要渲染的子集位置，升序。
    pub only: Vec<usize>,
}

/// 渲染器交回来的一页。`index` 是子集位置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedSlide<Img> {
    pub index: usize,
    pub image: Img,
    /// 该 kind 没有 Typst 版式，这一页是占位页。
    pub placeholder: bool,
    /// 内容缩到最小仍放不下，被裁切。
    pub overflow: bool,
}

impl<Img> RenderedSlide<Img> {
    pub fn new(index: usize, image: Img) -> Self {
        Self {
            index,
            image,
            placeholder: false,
            overflow: false,
        }
    }
}

/// 一批渲染结果。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PreviewResult<Img> {
    pub pages: Vec<RenderedSlide<Img>>,
    pub theme_fallback: Option<String>,
}

impl<Img> PreviewResult<Img> {
    pub fn new(pages: Vec<RenderedSlide<Img>>) -> Self {
        Self {
            pages,
            theme_fallback: None,
        }
    }
}

/// 一页缓存下来的图。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Cached<Img> {
    image: Img,
    placeholder: bool,
}

/// 缩略图缓存与调度器。`Img` 是界面侧的图片类型（GPUI 里是 `Arc<RenderImage>`）。
///
/// 下标约定与 Tauri 版一致：`image()` / `overflow()` 以**牌组下标**为键，
/// 请求与键的比较以**子集位置**为键（生成中有空位时两者不同）。
#[derive(Debug)]
pub struct PreviewCache<Img> {
    options: PreviewOptions,
    style: PreviewStyle,
    /// 牌组：`None` 表示该页还没有蓝图，不发给渲染器。
    deck: Vec<Option<Value>>,
    /// 只要这些**牌组下标**的图；`None` 表示全部。键里不含它，所以它不影响缓存有效性。
    wanted: Option<Vec<usize>>,
    /// 牌组下标 → 图。
    thumbs: HashMap<usize, Cached<Img>>,
    /// 牌组下标 → 是否被裁切。裁切是这一页自己的属性，即使图过期也要告诉调用方。
    overflow: HashMap<usize, bool>,
    /// 最近一次发请求时的键（按子集位置）。
    request_keys: Vec<String>,
    /// 最近一次发请求时的子集位置 → 牌组下标。
    request_indices: Vec<usize>,
    /// 子集位置 → 已显示的图来自哪一次请求。
    shown_sequence: HashMap<usize, u64>,
    /// 子集位置 → 已显示的图的键。
    shown_keys: HashMap<usize, String>,
    /// 子集位置 → 在途请求带的键。`reset` 之后 `reset_pending` 会让它不作数。
    pending_keys: HashMap<usize, String>,
    /// 牌组下标 → 最后一次被 `wanted` 点名的次序，淘汰用。
    last_used: HashMap<usize, u64>,
    access_counter: u64,
    in_flight: bool,
    /// 在途期间有过变化，请求结束后要重来一次。
    dirty: bool,
    /// `reset` 之后：下一次必须真的发请求，不能因为旧的 `pending_keys` 而跳过。
    reset_pending: bool,
    /// 每次 `reset` +1。
    generation: u64,
    /// 请求序号，用来判断响应新旧（`progressive`）与作废过期的尾部响应。
    sequence: u64,
    failed: bool,
    theme_fallback: Option<String>,
    /// 被淘汰或随牌组消失的图，攒在这里等界面层取走。`next_request` 也会产生释放，
    /// 而它的返回值里放不下——所以统一从这里 drain。
    recycled: Vec<Img>,
    /// 失败后要等「有东西变了」才重新请求。没有这道闸，调用方的
    /// `while next_request()` 循环会对一副编译不过的牌组永远重试。
    /// `set_deck` / `set_wanted` / `refresh` 相当于前端的 watcher，重新打开它。
    armed: bool,
}

impl<Img> PreviewCache<Img> {
    pub fn new(options: PreviewOptions) -> Self {
        Self {
            options,
            style: PreviewStyle::default(),
            deck: Vec::new(),
            wanted: None,
            thumbs: HashMap::new(),
            overflow: HashMap::new(),
            request_keys: Vec::new(),
            request_indices: Vec::new(),
            shown_sequence: HashMap::new(),
            shown_keys: HashMap::new(),
            pending_keys: HashMap::new(),
            last_used: HashMap::new(),
            access_counter: 0,
            in_flight: false,
            dirty: false,
            reset_pending: false,
            generation: 0,
            sequence: 0,
            failed: false,
            theme_fallback: None,
            recycled: Vec::new(),
            armed: true,
        }
    }

    /// 取出攒着的、要 `drop_image` 的图片。
    pub fn take_released(&mut self) -> Vec<Img> {
        std::mem::take(&mut self.recycled)
    }

    pub fn options(&self) -> &PreviewOptions {
        &self.options
    }

    /// 宽度向上取整到 64px 档位。
    pub fn bucket_width(width_px: f32) -> u32 {
        let width = width_px.max(1.0).ceil() as u32;
        WIDTH_BUCKET.max(width.div_ceil(WIDTH_BUCKET) * WIDTH_BUCKET)
    }

    /// 换牌组（换项目、生成中页面陆续到达）。旧图保留到新图落下，不闪成加载框。
    pub fn set_deck(&mut self, slides: &[Option<Value>], style: PreviewStyle) {
        self.deck = slides.to_vec();
        self.style = style;
        self.armed = true;
    }

    /// 只要这些**牌组下标**的图；空表示全部。
    pub fn set_wanted(&mut self, indices: &[usize]) {
        self.wanted = Some(indices.to_vec());
        self.armed = true;
    }

    /// 立刻按当前状态重新评估一次（不自动发请求，但保证下一次 `next_request` 会真的发出）。
    /// 渲染失败后由调用方调用它重试。
    pub fn refresh(&mut self) {
        self.dirty = true;
        self.armed = true;
    }

    /// 换目标：清空全部图并作废在途响应，调用方随即显示加载框。返回要交还的图片。
    pub fn reset(&mut self) -> Vec<Img> {
        self.generation += 1;
        self.reset_pending = true;
        self.deck.clear();
        self.wanted = None;
        self.request_keys.clear();
        self.request_indices.clear();
        self.shown_sequence.clear();
        self.shown_keys.clear();
        self.pending_keys.clear();
        self.last_used.clear();
        self.access_counter = 0;
        self.dirty = false;
        self.failed = false;
        self.armed = true;
        self.theme_fallback = None;
        self.overflow.clear();
        std::mem::take(&mut self.thumbs)
            .into_values()
            .map(|cached| cached.image)
            .collect()
    }

    /// 有请求在途，或刚被标记要重发。
    pub fn is_pending(&self) -> bool {
        self.in_flight || self.dirty
    }

    pub fn in_flight(&self) -> bool {
        self.in_flight
    }

    pub fn failed(&self) -> bool {
        self.failed
    }

    pub fn theme_fallback(&self) -> Option<&str> {
        self.theme_fallback.as_deref()
    }

    /// 这一页的图。键变了旧图仍在，直到新图落下。
    pub fn image(&self, index: usize) -> Option<&Img> {
        self.thumbs.get(&index).map(|cached| &cached.image)
    }

    pub fn placeholder(&self, index: usize) -> bool {
        self.thumbs
            .get(&index)
            .is_some_and(|cached| cached.placeholder)
    }

    pub fn overflow(&self, index: usize) -> bool {
        self.overflow.get(&index).copied().unwrap_or(false)
    }

    /// 这一页正在等一张更新的图（`SlideFrame.vue` 的「渲染中…」）。
    pub fn is_page_pending(&self, index: usize) -> bool {
        self.position_of(index)
            .is_some_and(|position| self.pending_keys.contains_key(&position))
    }

    pub fn cached(&self) -> usize {
        self.thumbs.len()
    }

    /// 算出这次要发什么；没有要发的返回 `None`。调用方执行后把结果交给 [`Self::complete`]，
    /// 并在还有后续时再次调用本方法。
    pub fn next_request(&mut self) -> Option<PreviewRequest> {
        if !self.options.enabled {
            self.dirty = false;
            return None;
        }
        if !self.armed {
            // 上一次失败了：等调用方改了内容、改了 `wanted`，或显式 `refresh()`。
            self.dirty = false;
            return None;
        }
        if self.in_flight {
            if self.options.serial {
                self.dirty = true;
            }
            return None;
        }
        // `reset` 作废了上一批在途响应，所以不能因为 `pending_keys` 里还留着位置就跳过。
        let ignoring_pending = self.reset_pending;
        self.reset_pending = false;
        self.dirty = true;

        let vanished = self.forget_vanished_pages();
        self.recycle(vanished);

        let all: Vec<usize> = self
            .deck
            .iter()
            .enumerate()
            .filter_map(|(index, slide)| slide.as_ref().map(|_| index))
            .collect();
        let asked: Option<HashSet<usize>> = self
            .wanted
            .as_ref()
            .map(|list| list.iter().copied().collect());
        let mut wanted_positions: Vec<usize> = Vec::new();
        let mut position_of: HashMap<usize, usize> = HashMap::new();
        for (position, index) in all.iter().enumerate() {
            position_of.insert(*index, position);
            if asked.as_ref().is_none_or(|asked| asked.contains(index)) {
                wanted_positions.push(position);
            }
        }
        for position in &wanted_positions {
            self.access_counter += 1;
            self.last_used.insert(all[*position], self.access_counter);
        }

        let width = Self::bucket_width(self.style.width_px);
        let deck: Vec<Value> = all
            .iter()
            .filter_map(|index| self.deck[*index].clone())
            .collect();
        // 键 = 蓝图 + 子集位置 + 主题 + 字体 + 宽度档位。
        self.request_keys = deck
            .iter()
            .enumerate()
            .map(|(position, slide)| {
                format!(
                    "{}|{}|{}|{}|{}",
                    serde_json::to_string(slide).unwrap_or_default(),
                    position,
                    self.style.theme.as_deref().unwrap_or(""),
                    self.style.fonts,
                    width
                )
            })
            .collect();
        self.request_indices = all.clone();

        let wanted_set: HashSet<usize> = wanted_positions.iter().copied().collect();
        let only: Vec<usize> = wanted_positions
            .iter()
            .copied()
            .filter(|position| {
                if !ignoring_pending && self.pending_keys.contains_key(position) {
                    // 在途请求带的就是这一页的当前键，等它回来即可。
                    return false;
                }
                self.shown_keys.get(position) != self.request_keys.get(*position)
            })
            .collect();
        if only.is_empty() {
            self.dirty = false;
            let evicted = self.evict(&wanted_set, &position_of);
            self.recycle(evicted);
            return None;
        }

        let id = self.sequence + 1;
        self.sequence = id;
        for &position in &only {
            self.pending_keys
                .insert(position, self.request_keys[position].clone());
        }
        let evicted = self.evict(&wanted_set, &position_of);
        self.recycle(evicted);
        self.in_flight = true;
        self.failed = false;
        Some(PreviewRequest {
            id,
            generation: self.generation,
            slides: deck,
            theme: self.style.theme.clone(),
            width_px: width,
            only,
        })
    }

    /// 请求执行完了。返回这一轮应当交还给界面的图片：被替换、被淘汰、过期作废的整批，
    /// 以及随牌组消失的页。
    pub fn complete(
        &mut self,
        request: &PreviewRequest,
        result: Result<PreviewResult<Img>, String>,
    ) -> Vec<Img> {
        let keys = self.request_keys.clone();
        let indices = self.request_indices.clone();
        let mut released = Vec::new();
        self.in_flight = false;

        let stale = request.generation != self.generation || request.id != self.sequence;
        match result {
            Ok(batch) => {
                if stale {
                    // 换过目标，或这个响应已经被更新的请求顶替：一张都不落，全部交还。
                    released.extend(batch.pages.into_iter().map(|page| page.image));
                } else {
                    self.theme_fallback = batch.theme_fallback;
                    for page in batch.pages {
                        let position = page.index;
                        // 裁切是这一页自己的属性：即使图过期也要让调用方知道。
                        if let Some(&index) = indices.get(position) {
                            self.overflow.insert(index, page.overflow);
                        }
                        let key = keys.get(position).cloned();
                        let current = key
                            .as_ref()
                            .is_some_and(|key| self.request_keys.get(position) == Some(key));
                        let newer = self.options.progressive
                            && self
                                .shown_sequence
                                .get(&position)
                                .is_none_or(|shown| request.id > *shown);
                        let Some(index) = indices.get(position).copied() else {
                            released.push(page.image);
                            continue;
                        };
                        if !current && !newer {
                            released.push(page.image);
                            continue;
                        }
                        if let Some(old) = self.thumbs.insert(
                            index,
                            Cached {
                                image: page.image,
                                placeholder: page.placeholder,
                            },
                        ) {
                            released.push(old.image);
                        }
                        self.shown_sequence.insert(position, request.id);
                        // 过期但更新的图：键记成触发它的那次请求的键，最新内容仍会被再请求。
                        self.shown_keys.insert(position, key.unwrap_or_default());
                    }
                }
            }
            Err(_) => {
                self.failed = true;
                self.armed = false;
                // 失败不接着请求：`refresh` 每次都会立起 `dirty`，失败的页键又没落下，
                // 接着请求只会原样再失败一次（一副编译不过的牌组会变成死循环）。
                self.dirty = false;
            }
        }

        if !stale {
            for &position in &request.only {
                if self.pending_keys.get(&position) == keys.get(position) {
                    self.pending_keys.remove(&position);
                }
            }
            let position_of: HashMap<usize, usize> = indices
                .iter()
                .enumerate()
                .map(|(position, index)| (*index, position))
                .collect();
            let asked: HashSet<usize> = self
                .deck
                .iter()
                .enumerate()
                .filter_map(|(index, slide)| slide.as_ref().map(|_| index))
                .filter(|index| self.wanted.as_ref().is_none_or(|list| list.contains(index)))
                .filter_map(|index| position_of.get(&index).copied())
                .collect();
            let evicted = self.evict(&asked, &position_of);
            self.recycle(evicted);
        }
        // `next_request` 造成的释放走 `take_released()`，这里返回的是本轮自己的，两条路互不重叠。
        released
    }

    /// 把一批要交还给界面的图片收进队列。
    fn recycle(&mut self, images: Vec<Img>) {
        self.recycled.extend(images);
    }

    /// 牌组里已经没有的页：图与溢出标记一并清掉。
    fn forget_vanished_pages(&mut self) -> Vec<Img> {
        if self.thumbs.is_empty() {
            return Vec::new();
        }
        let present: HashSet<usize> = self
            .deck
            .iter()
            .enumerate()
            .filter_map(|(index, slide)| slide.as_ref().map(|_| index))
            .collect();
        let vanished: Vec<usize> = self
            .thumbs
            .keys()
            .copied()
            .filter(|index| !present.contains(index))
            .collect();
        vanished
            .into_iter()
            .filter_map(|index| {
                self.overflow.remove(&index);
                self.last_used.remove(&index);
                self.thumbs.remove(&index).map(|cached| cached.image)
            })
            .collect()
    }

    /// 把缓存压回 `max_cached`：不在 `wanted` 里、也没有在途请求的图，按最久没被点名的先丢。
    /// 必须连「已显示键」一起忘掉，否则这一页再回到 `wanted` 时键还相等，永远不会重新请求。
    fn evict(
        &mut self,
        wanted_positions: &HashSet<usize>,
        position_of: &HashMap<usize, usize>,
    ) -> Vec<Img> {
        let Some(limit) = self.options.max_cached else {
            return Vec::new();
        };
        if self.thumbs.len() <= limit {
            return Vec::new();
        }
        let mut candidates: Vec<usize> = self
            .thumbs
            .keys()
            .copied()
            .filter(|index| match position_of.get(index) {
                None => true,
                Some(position) => {
                    !wanted_positions.contains(position)
                        && !self.pending_keys.contains_key(position)
                }
            })
            .collect();
        candidates.sort_by_key(|index| self.last_used.get(index).copied().unwrap_or(0));
        let mut released = Vec::new();
        for index in candidates {
            if self.thumbs.len() <= limit {
                break;
            }
            if let Some(cached) = self.thumbs.remove(&index) {
                released.push(cached.image);
            }
            self.last_used.remove(&index);
            self.overflow.remove(&index);
            if let Some(position) = position_of.get(&index) {
                self.shown_keys.remove(position);
                self.shown_sequence.remove(position);
            }
        }
        released
    }

    fn position_of(&self, index: usize) -> Option<usize> {
        self.request_indices
            .iter()
            .position(|candidate| *candidate == index)
    }
}

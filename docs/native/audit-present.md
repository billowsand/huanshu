# 演示覆盖层逐页审计（原版 Vue → 原生 GPUI）

范围：`src/views/PresentationOverlay.vue`（template + script）与 `.pres-*` 样式（`src/views/WorkflowView.vue:2671-3087`）→ `apps/huanshu-native/src/present.rs`。

- 状态：✅ 一致　🔧 本轮已修　⚠️ 有差距未修　➖ 有意不做（附原版行号依据）
- 截图：`docs/native/reference/present.png`（窗口态）对照 `docs/native/shell/present-full-0.png` / `present-full-hover-0.png`（全屏态）
- 审计时点：本轮 A 节（全屏铺满、浮动控制条、触摸滑动、Esc 语义）已在 `present.rs` 落地后审计。

## 1. `PresentationOverlay.vue` template → `present.rs::render`

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 覆盖层盖在当前页上、打开即全屏（被拒退回窗口态） | PresentationOverlay.vue:138-143、337-344 | `Presentation::new` | ✅ | 原版 `requestFullscreen`；原生 `toggle_fullscreen` |
| 标题栏在演示期间仍可见 | TitleBar.vue:122-127（z-index 10000）> WorkflowView.vue:2682（overlay 2000）；`docs/native/reference/present.png` 可见标题栏 | shell.rs 渲染顺序（标题栏在覆盖层之下一层） | ✅ | 原版覆盖层铺满窗口但标题栏 z-index 更高盖在上面；原生覆盖层渲染在标题栏之下，视觉等价（标题栏不透明） |
| 快捷键提示浮层（3 秒） | :370-379 + CSS :2707-2753 | render（`opened_at` + `SHORTCUT_HINT`） | ➖ | 原版是 5 个 `kbd` 芯片分条；原生合并为一行文字提示（`:263`），无 `kbd` 样式。属视觉形态差异，功能（提示内容、3 秒）一致 |
| 导航箭头（两侧圆形、点击翻页） | :382-383 + CSS :2755-2780 | `nav_arrow` | ✅ | 48px、16px 边距（原版 52px/1rem，见「视觉细节」） |
| **点箭头会连带触发舞台点击（一次跳两页）** | 原版 `.pres-nav` 与 `.pres-stage` 平级（:382-386），箭头点击不会到舞台 | render（本轮给箭头加 `.occlude()`） | 🔧 | 原生把箭头放进舞台内部，舞台 `on_mouse_down` 与箭头 `on_click` 都收到：右箭头一次 +2 页。本轮修：`.occlude()` 挡舞台命中，实测 +1/−1（`target/drive/bug-*`、`a1/a2-*`） |
| 舞台点击下一页 | :386、261-265 | render（`on_mouse_down`） | ✅ | `didSwipe` 吞点击本轮已接入（见触摸行） |
| 拉伸/等比舞台几何（`stretchedViewport` padH 96 / padV 88，全屏 0） | :51-83 | `stage_size` | 🔧 | 改前不管全屏都减控制条+标题栏；改后全屏铺满、非全屏减 96/88（按覆盖层区域口径，即原版口径再减原生多出的 36px 标题栏，见 `present.rs` 注释） |
| 舞台背景 | CSS :2782-2790 | render（`.bg(rgb(0x000000))`） | ➖ | 原版覆盖层背景是 bg + 顶部径向渐变 + 面板线性渐变（CSS :2672-2687）；原生舞台区纯黑底。视觉细节，渐变省略 |
| 触摸滑动翻页（\|dx\|>\|dy\| 且 \|dx\|>50，左滑下一页；didSwipe 吞随后的点击） | :37-41、267-281 | `on_swipe` + `Swipe` 状态机 | ✅ | 鼠标滚轮不触发；5 个单测。真机触摸未验证（本机无触摸屏）→ 人工验收 |
| 全览（O）：标题/副标题、按页比例缩略图、点击进入 | :412-441 + CSS :2792-2888 | `overview_grid` | ✅ | 原版缩略框 256×144 按页比例适配（`overviewBoxSize` :196-200）；原生 CARD_W 240 定宽、高按比例。见「视觉细节」 |
| 胶片条（页码方块、点击跳页、`title` 提示） | :446-457 + CSS :2977-3006 | render（`pres-filmstrip`） | ➖ | 跳页功能一致；原版每格有 `:title="{idx+1}. {bp.title}"` 悬停提示、36×28、圆角 4px；原生无 tooltip、高 26px、圆角 5px。视觉细节 |
| 页码 + 进度条 | :460-470 + CSS :3015-3044 | render | ✅ | 原生进度条容器 110px（原版 80px，见「视觉细节」） |
| 主题圆点（切换并保存） | :472 | render（`theme_dots`） | ✅ | |
| 全览/拉伸/全屏/退出按钮（title 文案逐字） | :474-489 | render（`icon_button`×4） | ✅ | 原版全览图标 `app-switcher`、等比图标 `crop`；原生用 `grid`/`fit-to-width`，同义图标（原 parity 已记） |
| 全屏浮动控制条（半透明 72%、上边框 45%、默认透明下移 8px、悬停淡入 0.3s、不占位） | :444 + CSS :3046-3066 | render（全屏分支） | 🔧 | 本轮实现：`with_animation` 0.3s 两端态动画（悬停标志拼进元素 id 驱动动画重跑）；`backdrop-filter` 模糊按任务省略；悬停挂在控制条自己（`on_hover` + `InputModalityIndependent`） |
| 非全屏控制条照旧底部占位 | CSS :2965-2975 | render（非全屏分支） | ✅ | 面板底、不透明、常显 |

## 2. `PresentationOverlay.vue` script（副作用与按键）→ `present.rs::on_key` / `ShellHandle`

| 项 | 原版位置 | 原生位置 | 状态 | 备注 |
|---|---|---|---|---|
| 按键：O / →↓空格 / ←↑ / Home / End / F / S | :295-335 | `on_key` | ✅ | 全览开时 ←→ 不翻页，照原版 |
| Esc：先关全览；**全屏时只退全屏不退出演示**；非全屏才退出 | :311-316 | `on_key` | 🔧 | 改前原生全屏 Esc 直接退出演示；改后与原版一致（实测 `target/drive/e1-e4`） |
| 退出演示：退出全屏并还原最大化 | :283-293、125-136 | `exit` → `ShellHandle::close_presentation` | ✅ | 原生无「最大化还原」概念（窗口态即原状） |
| 舞台位图：当前页先出图，再预取 -1…+3，`max_cached` 10 | :202-254 | `wanted` | ✅ | |
| 换页动画（forward/backward/jump 三段过渡） | CSS :2907-2963 | — | ➖ | parity 已记：GPUI 下先不做 |
| 触摸手柄（touchstart/touchend 直接监听） | :267-281 | `on_swipe`（`ScrollWheelEvent.touch_phase`） | ✅ | GPUI 把触摸平移转成带 `touch_phase` 的滚动事件 |

## 3. 视觉细节（归一行）

原版有、原生按 GPUI 省略或形态不同的：覆盖层顶部径向渐变 + 面板线性渐变背景；`.pres-nav` 的 `backdrop-filter` 模糊与阴影（原生纯色面板）；浮动控制条的 `backdrop-filter: blur(20px) saturate(1.4)`（任务允许省略）；`kbd` 芯片样式；幻灯片圆角 20px + 双层阴影的过渡容器（原生 `slide_frame` 无圆角阴影）；全览卡片 hover 上浮与阴影；切页动画。已做等价的：箭头/控制条半透明底色、边框 hover 色、胶片条 active 高亮。

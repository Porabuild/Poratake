//! AvatarGroup — port of `@heroui/react` v3.2.6 `AvatarGroup` and
//! `AvatarGroup.Count` (`avatar-group/avatar-group.tsx`,
//! `styles/components/avatar-group.css`).
//!
//! The group passes `size` (default `Md`), `color` and `variant` to its
//! **direct** [`Avatar`] children and to the count, filling only the props a
//! child omitted — a prop set on the child wins. An avatar nested inside some
//! other child element never inherits (v3 marks direct children with
//! `__avatar_group_child`). `max` keeps the first `max` children and appends
//! an automatic `+N` [`AvatarGroupCount`]; an explicit count is never
//! truncated and suppresses the automatic one. `is_grid` wraps with a 12px
//! gap and no overlap. Stacked siblings overlap by
//! `--avatar-group-overlap` (8px). The group authors no `role`, like v3.
//!
//! # Platform limitation: `overlap="clip"`
//!
//! v3's default clip mode cuts a transparent crescent out of every avatar
//! except the last with a CSS `mask-image: radial-gradient(...)`. The pinned
//! `gpui-pre` 0.3.11 has no alpha or shape mask: `ContentMask` is a
//! rectangle (`window.rs`, `pub struct ContentMask { bounds }`) and the scene
//! offers only quads, shadows, paths and sprites. The port therefore paints
//! the crescent instead of cutting it: every stacked avatar after the first
//! carries a `--avatar-group-seam` (2px) ring in the theme's `--background`
//! colour, which covers exactly the region the mask would remove from its
//! predecessor, plus v3's `padding-inline-end: overlap * .35` fallback nudge
//! on every avatar except the last. On a solid `--background` surface the
//! result matches; over an image, gradient or any other colour the seam is
//! that solid colour, not transparent. `Ring` is v3's own
//! `box-shadow: 0 0 0 2px var(--background)` on every stacked child.

use gpui::prelude::FluentBuilder;
use gpui::{
    px, AnyElement, App, ElementId, InteractiveElement, IntoElement, ParentElement, Pixels,
    RenderOnce, Styled, Window,
};
use herogpui_core::{element_id, Color, Size};
use herogpui_theme::ActiveTheme;

use crate::avatar::{Avatar, AvatarVariant, GroupDecor};

/// `overlap` — how stacked avatars separate visually. Ignored with
/// [`AvatarGroup::is_grid`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AvatarGroupOverlap {
    /// `.avatar-group--clip`: v3's transparent crescent seam, approximated
    /// with a `--background` seam (see the module docs).
    #[default]
    Clip,
    /// `.avatar-group--ring`: a solid 2px `--background` ring.
    Ring,
}

impl AvatarGroupOverlap {
    /// Every overlap style, in declaration order.
    pub const ALL: [AvatarGroupOverlap; 2] = [AvatarGroupOverlap::Clip, AvatarGroupOverlap::Ring];

    /// A human-readable label for this overlap style.
    pub fn label(self) -> &'static str {
        match self {
            AvatarGroupOverlap::Clip => "Clip",
            AvatarGroupOverlap::Ring => "Ring",
        }
    }

    fn modifier(self) -> &'static str {
        match self {
            AvatarGroupOverlap::Clip => "clip",
            AvatarGroupOverlap::Ring => "ring",
        }
    }
}

/// `AvatarGroup.Count` (`.avatar-group__count`): an [`Avatar`] whose fallback
/// is the count content, e.g. `+3`. Its own `size`/`color`/`variant` override
/// the group's; it is never truncated by [`AvatarGroup::max`].
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct AvatarGroupCount {
    id: ElementId,
    size: Option<Size>,
    color: Option<Color>,
    variant: Option<AvatarVariant>,
    children: Vec<AnyElement>,
}

impl AvatarGroupCount {
    /// Creates a count item with the given id.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            size: None,
            color: None,
            variant: None,
            children: Vec::new(),
        }
    }

    /// `children` — the count content, e.g. `+3`.
    pub fn child(mut self, content: impl IntoElement) -> Self {
        self.children.push(content.into_any_element());
        self
    }

    /// Overrides the group's `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = Some(size);
        self
    }

    /// Overrides the group's `color`.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Overrides the group's `variant`.
    pub fn variant(mut self, variant: AvatarVariant) -> Self {
        self.variant = Some(variant);
        self
    }

    /// Resolves the count into the [`Avatar`] it composes, with the group's
    /// props filling the omitted ones.
    fn into_avatar(
        self,
        size: Option<Size>,
        color: Option<Color>,
        variant: Option<AvatarVariant>,
    ) -> Avatar {
        let debug_id = self.id.clone();
        let mut avatar = Avatar::new(self.id);
        if let Some(size) = self.size {
            avatar = avatar.size(size);
        }
        if let Some(color) = self.color {
            avatar = avatar.color(color);
        }
        if let Some(variant) = self.variant {
            avatar = avatar.variant(variant);
        }
        avatar.inherit(size, color, variant).fallback(
            gpui::div()
                .flex()
                .items_center()
                .justify_center()
                .debug_selector(move || format!("avatar-group-count[{debug_id}]"))
                .children(self.children),
        )
    }
}

impl RenderOnce for AvatarGroupCount {
    /// Outside a group the count is a plain `Avatar` (v3's context defaults
    /// are all `undefined`).
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        self.into_avatar(None, None, None)
    }
}

/// One child of the group, in source order.
enum Member {
    /// A direct `Avatar`: inherits the group props and gets the overlap.
    Avatar(Box<Avatar>),
    /// Any other element: counted by `max`, but nothing inside inherits.
    Element(AnyElement),
}

/// HeroUI v3 `AvatarGroup`: a stacked or grid row of avatars with an
/// overflow count.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct AvatarGroup {
    id: ElementId,
    size: Size,
    color: Option<Color>,
    variant: Option<AvatarVariant>,
    max: Option<usize>,
    is_grid: bool,
    overlap: AvatarGroupOverlap,
    overlap_distance: Pixels,
    seam: Pixels,
    members: Vec<Member>,
    counts: Vec<AvatarGroupCount>,
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl AvatarGroup {
    /// The id scopes the automatic `+N` count's element id.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            size: Size::Md,
            color: None,
            variant: None,
            max: None,
            is_grid: false,
            overlap: AvatarGroupOverlap::Clip,
            overlap_distance: px(8.),
            seam: px(2.),
            members: Vec::new(),
            counts: Vec::new(),
            sx: None,
        }
    }

    /// `size` for children (and the count) that omit it. Default `Md`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// `color` for children (and the count) that omit it.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// `variant` for children (and the count) that omit it.
    pub fn variant(mut self, variant: AvatarVariant) -> Self {
        self.variant = Some(variant);
        self
    }

    /// `max` — render at most this many children and append an automatic
    /// `+N` count for the rest. Omit to show every child.
    pub fn max(mut self, max: usize) -> Self {
        self.max = Some(max);
        self
    }

    /// `isGrid` — a wrapping layout with a 12px gap and no overlap.
    pub fn is_grid(mut self, is_grid: bool) -> Self {
        self.is_grid = is_grid;
        self
    }

    /// `overlap` — [`AvatarGroupOverlap::Clip`] (default) or `Ring`.
    pub fn overlap(mut self, overlap: AvatarGroupOverlap) -> Self {
        self.overlap = overlap;
        self
    }

    /// `--avatar-group-overlap` (default 8px): how far each stacked sibling
    /// pulls under its predecessor.
    pub fn overlap_distance(mut self, distance: impl Into<Pixels>) -> Self {
        self.overlap_distance = distance.into();
        self
    }

    /// `--avatar-group-seam` (default 2px): the ring / seam width.
    pub fn seam(mut self, seam: impl Into<Pixels>) -> Self {
        self.seam = seam.into();
        self
    }

    /// A direct `Avatar` child.
    pub fn child(mut self, avatar: Avatar) -> Self {
        self.members.push(Member::Avatar(Box::new(avatar)));
        self
    }

    /// Several direct `Avatar` children.
    pub fn children(mut self, avatars: impl IntoIterator<Item = Avatar>) -> Self {
        self.members
            .extend(avatars.into_iter().map(|a| Member::Avatar(Box::new(a))));
        self
    }

    /// A non-avatar child. It counts toward `max`, but avatars nested inside
    /// it do not inherit the group props.
    pub fn child_element(mut self, element: impl IntoElement) -> Self {
        self.members
            .push(Member::Element(element.into_any_element()));
        self
    }

    /// An explicit `AvatarGroup.Count`: never truncated, and it suppresses
    /// the automatic `+N`.
    pub fn count(mut self, count: AvatarGroupCount) -> Self {
        self.counts.push(count);
        self
    }

    /// The one slot for caller-owned low-level styling of the group root.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }
}

impl RenderOnce for AvatarGroup {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let background = cx.colors().background;
        let (size, color, variant) = (Some(self.size), self.color, self.variant);

        let total = self.members.len();
        let mut members = self.members;
        if let Some(max) = self.max {
            members.truncate(max);
        }
        let remaining = total - members.len();

        // Resolve every child into render order: visible members, then the
        // explicit counts or the automatic `+N`.
        let mut items: Vec<Member> = members
            .into_iter()
            .map(|member| match member {
                Member::Avatar(avatar) => {
                    Member::Avatar(Box::new(avatar.inherit(size, color, variant)))
                }
                other => other,
            })
            .collect();
        if !self.counts.is_empty() {
            items.extend(
                self.counts
                    .into_iter()
                    .map(|c| Member::Avatar(Box::new(c.into_avatar(size, color, variant)))),
            );
        } else if remaining > 0 {
            let auto = AvatarGroupCount::new(element_id::scoped(&self.id, "count"))
                .child(format!("+{remaining}"));
            items.push(Member::Avatar(Box::new(
                auto.into_avatar(size, color, variant),
            )));
        }

        let stacked = !self.is_grid;
        let last = items.len().saturating_sub(1);
        let mut prev_is_avatar = false;
        let mut children: Vec<AnyElement> = Vec::with_capacity(items.len());
        for (i, item) in items.into_iter().enumerate() {
            match item {
                Member::Avatar(avatar) => {
                    // `.avatar + .avatar` / `.avatar + .avatar-group__count`.
                    let follows = stacked && prev_is_avatar;
                    let seam = match self.overlap {
                        AvatarGroupOverlap::Ring => stacked,
                        // Painted crescent: the seam around this avatar
                        // covers the cut v3 masks out of its predecessor.
                        AvatarGroupOverlap::Clip => follows,
                    };
                    let decor = GroupDecor {
                        margin_start: follows.then_some(self.overlap_distance),
                        seam: seam.then_some((self.seam, background)),
                        fallback_pad_end: (stacked
                            && self.overlap == AvatarGroupOverlap::Clip
                            && i != last)
                            .then(|| self.overlap_distance * 0.35),
                    };
                    children.push(avatar.group_decor(decor).into_any_element());
                    prev_is_avatar = true;
                }
                Member::Element(element) => {
                    children.push(element);
                    prev_is_avatar = false;
                }
            }
        }

        let debug_id = self.id.clone();
        let grid_key = if self.is_grid {
            ".avatar-group--grid"
        } else {
            ""
        };
        let overlap_key = self.overlap.modifier();
        let root = gpui::div()
            .flex()
            .items_center()
            .debug_selector(move || {
                format!("avatar-group[{debug_id}]{grid_key}.avatar-group--{overlap_key}")
            })
            .when(self.is_grid, |el| el.flex_wrap().gap(px(12.)))
            .children(children);
        crate::util::apply_sx(root, &self.sx)
    }
}

crate::util::impl_component_styled!(AvatarGroup);

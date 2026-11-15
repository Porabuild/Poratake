//! A named, embedded Lucide icon set (HeroGPUI extension, not a HeroUI v3
//! API).
//!
//! [`IconName`] names a curated subset of [Lucide](https://lucide.dev)
//! icons, and [`Icon`] draws one at a size and colour. The SVGs are copied
//! verbatim from the official `lucide-static` [`LUCIDE_VERSION`] release
//! into this crate's `assets/lucide/` and compiled in with `include_bytes!`,
//! so [`HeroGpuiAssets`](crate::HeroGpuiAssets) serves every variant with no
//! file or feature to set up:
//!
//! ```
//! use herogpui_components::{Icon, IconName, IconSize};
//! use gpui::{px, rgb};
//!
//! let _search = Icon::new(IconName::Search);
//! let _big_red = Icon::new(IconName::Heart).size(px(24.)).color(rgb(0xe11d48));
//! // A size step (`Sizable`), and Lucide's stroke controls.
//! let _thin = Icon::new(IconName::Star)
//!     .size(IconSize::Lg)
//!     .stroke_width(1.5)
//!     .absolute_stroke_width(true);
//!
//! // Any builder that takes an icon path takes a name too.
//! let path: gpui::SharedString = IconName::Folder.into();
//! assert_eq!(path.as_ref(), "herogpui/icons/lucide/folder.svg");
//! ```
//!
//! The chrome paths in [`icons`](crate::icons) are separate and unchanged:
//! those are HeroUI's own glyphs, drawn by the components, and keep their
//! `herogpui/icons/<name>.svg` paths. Lucide icons live one level down, under
//! [`LUCIDE_ICON_PREFIX`], so the two sets never collide.
//!
//! Lucide is ISC-licensed, and a few of its icons derive from Feather (MIT);
//! the full text ships beside the SVGs as `assets/lucide/LICENSE` and the
//! attribution is in this crate's `NOTICE`.
//!
//! # Stroke width
//!
//! gpui rasterises an SVG as one alpha mask painted in a single colour, so
//! a stroke width cannot be styled after the fact. [`Icon::stroke_width`]
//! therefore picks a different asset: [`IconName::path_with_stroke_width`]
//! appends `?stroke-width=<w>` to the icon's path, and
//! [`HeroGpuiAssets`](crate::HeroGpuiAssets) serves that path as the same
//! Lucide file with its root `stroke-width` rewritten. gpui caches the
//! rasterised mask per path and size, so each width is rendered once. It
//! applies to the Lucide set only: an [`Icon::from_path`] SVG is drawn as its
//! asset source serves it, and an application that serves Lucide paths from
//! its own asset source first must forward the query to `HeroGpuiAssets`.
//!
//! # Adding an icon
//!
//! The set is the list in `.shots/lucide-icons.txt`. Add a name there and run
//! `python3 .shots/sync-lucide.py`: it downloads the pinned `lucide-static`
//! tarball (checking its pinned SHA-512), copies the listed icons byte for
//! byte into `assets/lucide/` with the license, and regenerates the variant
//! list below and [`LUCIDE_VERSION`]. `python3 .shots/sync-lucide.py --check`
//! (run by CI's parity job) fails when the list, the files and this module
//! disagree; `tests/icon.rs` fails for a variant without a file and for a
//! file without a variant.

use gpui::{prelude::*, px, svg, App, Hsla, IntoElement, Pixels, RenderOnce, SharedString, Window};
use herogpui_theme::ActiveTheme;

/// The `lucide-static` release the embedded SVGs were copied from.
pub const LUCIDE_VERSION: &str = "1.31.0";

/// The asset-path directory every [`IconName::path`] lives in.
pub const LUCIDE_ICON_PREFIX: &str = "herogpui/icons/lucide";

/// The size [`Icon`] draws at unless told otherwise — the 16px the
/// components' own chrome icons use.
pub const DEFAULT_ICON_SIZE: Pixels = px(16.);

/// The stroke width, in the icons' 24-unit `viewBox`, that every Lucide SVG
/// ships with (Lucide's `strokeWidth` default).
pub const DEFAULT_STROKE_WIDTH: f32 = 2.;

/// An [`Icon`] size step, for [`Sizable`](crate::Sizable) (HeroGPUI
/// extension). The steps are the Tailwind `size-*` squares icons take in
/// HeroUI's own markup: `Xs` 12px, `Sm` 14px, `Md` 16px (the default,
/// [`DEFAULT_ICON_SIZE`]), `Lg` 20px and `Xl` 24px. It converts into
/// [`Pixels`], so [`Icon::size`] takes a step or any pixel length.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum IconSize {
    /// 12px (`size-3`).
    Xs,
    /// 14px (`size-3.5`).
    Sm,
    /// 16px (`size-4`), the default.
    #[default]
    Md,
    /// 20px (`size-5`).
    Lg,
    /// 24px (`size-6`), Lucide's own default.
    Xl,
}

impl IconSize {
    /// Every step, smallest first.
    pub const ALL: [IconSize; 5] = [
        IconSize::Xs,
        IconSize::Sm,
        IconSize::Md,
        IconSize::Lg,
        IconSize::Xl,
    ];

    /// The step's width and height.
    pub const fn pixels(self) -> Pixels {
        match self {
            IconSize::Xs => px(12.),
            IconSize::Sm => px(14.),
            IconSize::Md => px(16.),
            IconSize::Lg => px(20.),
            IconSize::Xl => px(24.),
        }
    }
}

impl From<IconSize> for Pixels {
    fn from(size: IconSize) -> Self {
        size.pixels()
    }
}

macro_rules! lucide_icons {
    ($($variant:ident => $name:literal),* $(,)?) => {
        /// A Lucide icon embedded in this crate (HeroGPUI extension).
        ///
        /// Each variant is the Lucide icon of the same name in PascalCase
        /// (`circle-check` is [`IconName::CircleCheck`]). The set is
        /// curated, not all of Lucide, and grows in minor releases, which is
        /// why the enum is `#[non_exhaustive]`.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[non_exhaustive]
        pub enum IconName {
            $(
                #[doc = concat!("Lucide `", $name, "`.")]
                $variant,
            )*
        }

        impl IconName {
            /// Every icon, in name order.
            pub const ALL: &'static [IconName] = &[$(IconName::$variant),*];

            /// Lucide's own kebab-case name, e.g. `"circle-check"`.
            pub const fn name(self) -> &'static str {
                match self {
                    $(IconName::$variant => $name,)*
                }
            }

            /// The asset path [`HeroGpuiAssets`](crate::HeroGpuiAssets)
            /// serves this icon under, e.g.
            /// `"herogpui/icons/lucide/circle-check.svg"`.
            pub const fn path(self) -> &'static str {
                match self {
                    $(IconName::$variant => concat!("herogpui/icons/lucide/", $name, ".svg"),)*
                }
            }

            /// The embedded SVG document, byte-identical to `lucide-static`.
            pub const fn svg(self) -> &'static [u8] {
                match self {
                    $(IconName::$variant => include_bytes!(concat!("../assets/lucide/", $name, ".svg")),)*
                }
            }
        }
    };
}

lucide_icons! {
    Activity => "activity",
    AlarmClock => "alarm-clock",
    Archive => "archive",
    ArrowDown => "arrow-down",
    ArrowDownLeft => "arrow-down-left",
    ArrowDownRight => "arrow-down-right",
    ArrowDownToLine => "arrow-down-to-line",
    ArrowLeft => "arrow-left",
    ArrowLeftRight => "arrow-left-right",
    ArrowRight => "arrow-right",
    ArrowUp => "arrow-up",
    ArrowUpDown => "arrow-up-down",
    ArrowUpLeft => "arrow-up-left",
    ArrowUpRight => "arrow-up-right",
    ArrowUpToLine => "arrow-up-to-line",
    AtSign => "at-sign",
    Ban => "ban",
    Battery => "battery",
    Bell => "bell",
    BellOff => "bell-off",
    Bold => "bold",
    Book => "book",
    BookOpen => "book-open",
    Bookmark => "bookmark",
    Bot => "bot",
    Box => "box",
    Briefcase => "briefcase",
    Bug => "bug",
    Building => "building",
    Calendar => "calendar",
    CalendarDays => "calendar-days",
    Camera => "camera",
    ChartBar => "chart-bar",
    ChartColumn => "chart-column",
    ChartLine => "chart-line",
    ChartPie => "chart-pie",
    Check => "check",
    CheckCheck => "check-check",
    ChevronDown => "chevron-down",
    ChevronLeft => "chevron-left",
    ChevronRight => "chevron-right",
    ChevronUp => "chevron-up",
    ChevronsDown => "chevrons-down",
    ChevronsLeft => "chevrons-left",
    ChevronsRight => "chevrons-right",
    ChevronsUp => "chevrons-up",
    ChevronsUpDown => "chevrons-up-down",
    Circle => "circle",
    CircleAlert => "circle-alert",
    CircleCheck => "circle-check",
    CircleDot => "circle-dot",
    CircleHelp => "circle-help",
    CircleMinus => "circle-minus",
    CirclePlus => "circle-plus",
    CircleUser => "circle-user",
    CircleX => "circle-x",
    Clipboard => "clipboard",
    ClipboardCheck => "clipboard-check",
    ClipboardCopy => "clipboard-copy",
    Clock => "clock",
    Cloud => "cloud",
    CloudDownload => "cloud-download",
    CloudUpload => "cloud-upload",
    Code => "code",
    CodeXml => "code-xml",
    Columns2 => "columns-2",
    Command => "command",
    Compass => "compass",
    Contrast => "contrast",
    Copy => "copy",
    CornerDownLeft => "corner-down-left",
    Cpu => "cpu",
    CreditCard => "credit-card",
    Crop => "crop",
    Database => "database",
    DollarSign => "dollar-sign",
    Download => "download",
    Droplet => "droplet",
    Ellipsis => "ellipsis",
    EllipsisVertical => "ellipsis-vertical",
    Expand => "expand",
    ExternalLink => "external-link",
    Eye => "eye",
    EyeOff => "eye-off",
    File => "file",
    FileCode => "file-code",
    FilePlus => "file-plus",
    FileText => "file-text",
    Files => "files",
    Film => "film",
    Flag => "flag",
    Folder => "folder",
    FolderOpen => "folder-open",
    FolderPlus => "folder-plus",
    Funnel => "funnel",
    Gift => "gift",
    GitBranch => "git-branch",
    GitCommitHorizontal => "git-commit-horizontal",
    GitMerge => "git-merge",
    GitPullRequest => "git-pull-request",
    Globe => "globe",
    GraduationCap => "graduation-cap",
    Grid2x2 => "grid-2x2",
    GripHorizontal => "grip-horizontal",
    GripVertical => "grip-vertical",
    HardDrive => "hard-drive",
    Hash => "hash",
    Headphones => "headphones",
    Heart => "heart",
    History => "history",
    House => "house",
    Image => "image",
    Inbox => "inbox",
    Info => "info",
    Italic => "italic",
    Key => "key",
    Keyboard => "keyboard",
    Languages => "languages",
    Laptop => "laptop",
    Layers => "layers",
    LayoutDashboard => "layout-dashboard",
    LayoutGrid => "layout-grid",
    LayoutList => "layout-list",
    Lightbulb => "lightbulb",
    Link => "link",
    Link2 => "link-2",
    List => "list",
    ListChecks => "list-checks",
    ListFilter => "list-filter",
    ListOrdered => "list-ordered",
    Loader => "loader",
    LoaderCircle => "loader-circle",
    Lock => "lock",
    LockOpen => "lock-open",
    LogIn => "log-in",
    LogOut => "log-out",
    Mail => "mail",
    Map => "map",
    MapPin => "map-pin",
    Maximize => "maximize",
    Maximize2 => "maximize-2",
    Menu => "menu",
    MessageCircle => "message-circle",
    MessageSquare => "message-square",
    Mic => "mic",
    MicOff => "mic-off",
    Minimize => "minimize",
    Minimize2 => "minimize-2",
    Minus => "minus",
    Monitor => "monitor",
    Moon => "moon",
    MousePointer => "mouse-pointer",
    Move => "move",
    OctagonAlert => "octagon-alert",
    Package => "package",
    Palette => "palette",
    PanelLeft => "panel-left",
    PanelRight => "panel-right",
    Paperclip => "paperclip",
    Pause => "pause",
    Pencil => "pencil",
    Phone => "phone",
    Pin => "pin",
    Pipette => "pipette",
    Play => "play",
    Plus => "plus",
    Power => "power",
    Printer => "printer",
    Puzzle => "puzzle",
    Quote => "quote",
    Redo2 => "redo-2",
    RefreshCw => "refresh-cw",
    Repeat => "repeat",
    Rocket => "rocket",
    RotateCcw => "rotate-ccw",
    RotateCw => "rotate-cw",
    Rows2 => "rows-2",
    Rss => "rss",
    Save => "save",
    Scissors => "scissors",
    Search => "search",
    Send => "send",
    Server => "server",
    Settings => "settings",
    Settings2 => "settings-2",
    Share => "share",
    Share2 => "share-2",
    Shield => "shield",
    ShieldCheck => "shield-check",
    ShoppingBag => "shopping-bag",
    ShoppingCart => "shopping-cart",
    Shrink => "shrink",
    Shuffle => "shuffle",
    SkipBack => "skip-back",
    SkipForward => "skip-forward",
    SlidersHorizontal => "sliders-horizontal",
    Smartphone => "smartphone",
    Smile => "smile",
    Sparkles => "sparkles",
    Square => "square",
    SquareCheck => "square-check",
    SquarePen => "square-pen",
    Star => "star",
    Strikethrough => "strikethrough",
    Sun => "sun",
    SunMoon => "sun-moon",
    Table => "table",
    Tablet => "tablet",
    Tag => "tag",
    Tags => "tags",
    Terminal => "terminal",
    TextAlignCenter => "text-align-center",
    TextAlignEnd => "text-align-end",
    TextAlignStart => "text-align-start",
    ThumbsDown => "thumbs-down",
    ThumbsUp => "thumbs-up",
    Timer => "timer",
    Trash => "trash",
    Trash2 => "trash-2",
    TrendingDown => "trending-down",
    TrendingUp => "trending-up",
    TriangleAlert => "triangle-alert",
    Truck => "truck",
    Type => "type",
    Underline => "underline",
    Undo2 => "undo-2",
    Unlink => "unlink",
    Upload => "upload",
    User => "user",
    UserCheck => "user-check",
    UserMinus => "user-minus",
    UserPlus => "user-plus",
    Users => "users",
    Video => "video",
    Volume2 => "volume-2",
    VolumeX => "volume-x",
    Wallet => "wallet",
    WandSparkles => "wand-sparkles",
    Wifi => "wifi",
    WifiOff => "wifi-off",
    Wrench => "wrench",
    X => "x",
    Zap => "zap",
    ZoomIn => "zoom-in",
    ZoomOut => "zoom-out",
}

impl IconName {
    /// The icon Lucide calls `name` (kebab-case, as in [`IconName::name`]).
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .binary_search_by(|icon| icon.name().cmp(name))
            .ok()
            .map(|index| Self::ALL[index])
    }

    /// The asset path of this icon drawn with `width` as its stroke width
    /// (in the 24-unit `viewBox`; Lucide's default is
    /// [`DEFAULT_STROKE_WIDTH`]): [`path`](Self::path) with a
    /// `?stroke-width=<w>` query that
    /// [`HeroGpuiAssets`](crate::HeroGpuiAssets) serves as the rewritten
    /// file. The width is rounded to three decimals and negative widths
    /// count as zero; the default width, and a non-finite one, give the
    /// plain path.
    pub fn path_with_stroke_width(self, width: f32) -> SharedString {
        match stroke_width_query(width) {
            Some(width) => format!("{}?stroke-width={width}", self.path()).into(),
            None => self.into(),
        }
    }

    /// The icon whose [`IconName::path`] is `path`.
    pub fn from_path(path: &str) -> Option<Self> {
        path.strip_prefix(LUCIDE_ICON_PREFIX)?
            .strip_prefix('/')?
            .strip_suffix(".svg")
            .and_then(Self::from_name)
    }
}

/// `width` spelled for a stroke-width query: rounded to three decimals with
/// trailing zeros trimmed, or `None` for the default or a non-finite width.
fn stroke_width_query(width: f32) -> Option<String> {
    if !width.is_finite() {
        return None;
    }
    let rounded = (width.max(0.) * 1000.).round() / 1000.;
    if (rounded - DEFAULT_STROKE_WIDTH).abs() < 0.0005 {
        return None;
    }
    let text = format!("{rounded:.3}");
    Some(text.trim_end_matches('0').trim_end_matches('.').to_owned())
}

/// The SVG [`HeroGpuiAssets`](crate::HeroGpuiAssets) serves for `path`
/// when it is a Lucide path with a `?stroke-width=` query: the icon's file
/// with its root `stroke-width` replaced.
pub(crate) fn stroked_svg(path: &str) -> Option<Vec<u8>> {
    let (base, width) = path.split_once("?stroke-width=")?;
    let width: f32 = width
        .parse()
        .ok()
        .filter(|w: &f32| w.is_finite() && *w >= 0.)?;
    let icon = IconName::from_path(base)?;
    let svg = std::str::from_utf8(icon.svg()).ok()?;
    let from = format!("stroke-width=\"{DEFAULT_STROKE_WIDTH}\"");
    let to = format!("stroke-width=\"{width}\"");
    svg.contains(&from)
        .then(|| svg.replacen(&from, &to, 1).into_bytes())
}

impl From<IconName> for SharedString {
    fn from(icon: IconName) -> Self {
        SharedString::new_static(icon.path())
    }
}

impl From<IconName> for Icon {
    fn from(icon: IconName) -> Self {
        Icon::new(icon)
    }
}

/// One icon, drawn as a GPUI `svg()` (HeroGPUI extension).
///
/// Sized [`DEFAULT_ICON_SIZE`] and painted in the theme's `foreground`
/// unless told otherwise — gpui's `svg()` does not inherit the surrounding
/// text colour, so an icon on an accent or danger fill needs `.color(..)`.
/// The icon is decorative and reports no accessibility node, as
/// `lucide-react` marks its `<svg>` `aria-hidden` when it is given no label:
/// name the control that holds it instead.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Icon {
    path: SharedString,
    size: Pixels,
    color: Option<Hsla>,
    stroke_width: Option<f32>,
    absolute_stroke_width: bool,
}

impl Icon {
    /// An embedded Lucide icon.
    pub fn new(icon: IconName) -> Self {
        Self::from_path(icon)
    }

    /// Any SVG the registered asset source serves: one of the
    /// [`icons`](crate::icons) chrome paths, or an application's own.
    pub fn from_path(path: impl Into<SharedString>) -> Self {
        Self {
            path: path.into(),
            size: DEFAULT_ICON_SIZE,
            color: None,
            stroke_width: None,
            absolute_stroke_width: false,
        }
    }

    /// The width and height of the square the icon draws in: a pixel length
    /// or an [`IconSize`] step (also through [`Sizable`](crate::Sizable)).
    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = size.into();
        self
    }

    /// Lucide's `strokeWidth`: the stroke width in the icon's 24-unit
    /// `viewBox` (default [`DEFAULT_STROKE_WIDTH`]), so it scales with the
    /// icon. Lucide icons only; see the [module docs](self#stroke-width).
    pub fn stroke_width(mut self, width: f32) -> Self {
        self.stroke_width = Some(width);
        self
    }

    /// Lucide's `absoluteStrokeWidth`: keep the stroke
    /// [`stroke_width`](Self::stroke_width) pixels wide at any size, as
    /// Lucide does by scaling it by `24 / size`.
    pub fn absolute_stroke_width(mut self, absolute: bool) -> Self {
        self.absolute_stroke_width = absolute;
        self
    }

    /// The asset path to draw: the Lucide path for the effective stroke
    /// width, or the path as given.
    fn served_path(&self) -> SharedString {
        match IconName::from_path(&self.path) {
            Some(icon) if self.stroke_width.is_some() || self.absolute_stroke_width => {
                icon.path_with_stroke_width(self.effective_stroke_width())
            }
            _ => self.path.clone(),
        }
    }

    /// The stroke width to serve, in `viewBox` units.
    fn effective_stroke_width(&self) -> f32 {
        let width = self.stroke_width.unwrap_or(DEFAULT_STROKE_WIDTH);
        let size = f32::from(self.size);
        if self.absolute_stroke_width && size > 0. {
            width * 24. / size
        } else {
            width
        }
    }

    /// The paint colour (default: the theme's `foreground`).
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }
}

impl RenderOnce for Icon {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let color = self.color.unwrap_or(cx.colors().foreground);
        let path = self.served_path();
        svg()
            .path(path)
            .size(self.size)
            .flex_none()
            .text_color(color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stroke_width_selects_the_served_path() {
        let plain = Icon::new(IconName::Star);
        assert_eq!(plain.served_path().as_ref(), IconName::Star.path());
        let thin = Icon::new(IconName::Star).stroke_width(1.5);
        assert_eq!(
            thin.served_path().as_ref(),
            "herogpui/icons/lucide/star.svg?stroke-width=1.5"
        );
        // The default width needs no rewrite.
        let two = Icon::new(IconName::Star).stroke_width(2.);
        assert_eq!(two.served_path().as_ref(), IconName::Star.path());
        // Absolute: 2px at 48px is 1 viewBox unit, at 12px it is 4.
        let big = Icon::new(IconName::Star)
            .size(px(48.))
            .absolute_stroke_width(true);
        assert_eq!(
            big.served_path().as_ref(),
            "herogpui/icons/lucide/star.svg?stroke-width=1"
        );
        let small = Icon::new(IconName::Star)
            .size(IconSize::Xs)
            .stroke_width(2.)
            .absolute_stroke_width(true);
        assert_eq!(
            small.served_path().as_ref(),
            "herogpui/icons/lucide/star.svg?stroke-width=4"
        );
        // A path that is not a Lucide icon is drawn as given.
        let chrome = Icon::from_path(crate::icons::CHECK).stroke_width(1.);
        assert_eq!(chrome.served_path().as_ref(), crate::icons::CHECK);
    }

    #[test]
    fn stroke_width_queries_round_and_reject_bad_widths() {
        assert_eq!(stroke_width_query(1.23456).as_deref(), Some("1.235"));
        assert_eq!(stroke_width_query(3.).as_deref(), Some("3"));
        assert_eq!(stroke_width_query(-1.).as_deref(), Some("0"));
        assert_eq!(stroke_width_query(2.0001), None);
        assert_eq!(stroke_width_query(f32::NAN), None);
        assert!(stroked_svg("herogpui/icons/lucide/star.svg?stroke-width=abc").is_none());
        assert!(stroked_svg("herogpui/icons/lucide/nope.svg?stroke-width=1").is_none());
        assert!(stroked_svg("herogpui/icons/check.svg?stroke-width=1").is_none());
    }
}

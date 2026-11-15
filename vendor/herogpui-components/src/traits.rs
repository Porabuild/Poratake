//! Shared builder traits (HeroGPUI extension, not a HeroUI v3 API).
//!
//! HeroUI v3 spells `isDisabled`, `size` and `isSelected` on every component
//! that has them, and this port keeps those spellings as **inherent** builder
//! methods — the parity audits read them there. These traits add a common
//! name on top, so generic code can configure any component that supports the
//! prop without naming its concrete type:
//!
//! ```
//! use herogpui_components::{Button, Disableable, Sizable, Size, Switch};
//!
//! fn compact<T: Disableable + Sizable<Size = Size>>(component: T, busy: bool) -> T {
//!     component.is_disabled(busy).size(Size::Sm)
//! }
//!
//! let _button = compact(Button::new("save").label("Save"), true);
//! let _switch = compact(Switch::new("wifi"), false);
//! ```
//!
//! Every implementation delegates to the inherent builder of the same name,
//! so a call through the trait and a direct call are the same code path.
//! The pattern follows gpui-kit's `Disableable`/`Sizable`/`Selectable`
//! (Apache-2.0, Longbridge), rewritten for HeroUI's `is_*` naming.

use crate::*;

/// A component with v3's `isDisabled` prop.
pub trait Disableable: Sized {
    /// `isDisabled` — whether the component is disabled.
    fn is_disabled(self, disabled: bool) -> Self;
}

/// A component with a `size` prop. [`Sizable::Size`] is the component's own
/// scale: HeroUI's [`Size`] for most, a component-specific enum where v3
/// defines one (for example [`ModalSize`]).
pub trait Sizable: Sized {
    /// The size scale this component accepts.
    type Size;
    /// `size` — the component's size step.
    fn size(self, size: Self::Size) -> Self;
}

/// A component with v3's controlled `isSelected` prop.
pub trait Selectable: Sized {
    /// `isSelected` — the controlled selection state.
    fn is_selected(self, selected: bool) -> Self;
}

macro_rules! disableable {
    ($($ty:ident),* $(,)?) => {$(
        impl Disableable for $ty {
            fn is_disabled(self, disabled: bool) -> Self {
                $ty::is_disabled(self, disabled)
            }
        }
    )*};
}

macro_rules! sizable {
    ($($ty:ident => $size:ty),* $(,)?) => {$(
        impl Sizable for $ty {
            type Size = $size;
            fn size(self, size: $size) -> Self {
                $ty::size(self, size)
            }
        }
    )*};
}

macro_rules! selectable {
    ($($ty:ident),* $(,)?) => {$(
        impl Selectable for $ty {
            fn is_selected(self, selected: bool) -> Self {
                $ty::is_selected(self, selected)
            }
        }
    )*};
}

disableable!(
    AccordionItem,
    Accordion,
    Autocomplete,
    Breadcrumbs,
    Button,
    ButtonGroup,
    Calendar,
    Checkbox,
    CheckboxOption,
    CheckboxGroup,
    CloseButton,
    ColorArea,
    ColorField,
    ColorPicker,
    ColorSlider,
    ColorSwatch,
    ColorSwatchPicker,
    ComboBox,
    DateField,
    DatePicker,
    DateRangePicker,
    Disclosure,
    DisclosureGroup,
    HoverCard,
    Label,
    Input,
    TextField,
    SearchField,
    InputGroup,
    InputOTP,
    Link,
    ListBoxItem,
    NumberField,
    Pagination,
    RadioOption,
    RadioGroup,
    RangeCalendar,
    ResizablePanelGroup,
    Select,
    Sidebar,
    SidebarItem,
    Slider,
    Switch,
    TabItem,
    Tabs,
    Tag,
    TagGroup,
    TextArea,
    TimeField,
    ToggleButton,
    ToggleButtonGroup,
    Tooltip,
    TreeItem,
    TreeView,
);

sizable!(
    AlertDialog => AlertDialogSize,
    Avatar => Size,
    AvatarGroupCount => Size,
    AvatarGroup => Size,
    Badge => Size,
    Button => Size,
    ButtonGroup => Size,
    Checkbox => CheckboxSize,
    Chip => Size,
    Icon => IconSize,
    ColorSwatch => SizeXl,
    ColorSwatchPicker => SizeXl,
    Meter => Size,
    Modal => ModalSize,
    Pagination => Size,
    ProgressBar => Size,
    ProgressCircle => Size,
    RadioGroup => RadioSize,
    Slider => SliderSize,
    Spinner => SpinnerSize,
    Switch => Size,
    Tabs => TabsSize,
    TagGroup => Size,
    ToggleButton => Size,
    ToggleButtonGroup => Size,
    Toolbar => Size,
);

selectable!(Checkbox, Switch, ToggleButton);

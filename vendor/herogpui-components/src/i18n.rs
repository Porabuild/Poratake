//! Localizable component chrome strings (HeroGPUI extension).
//!
//! Dates, times and numbers already format per locale through ICU4X (each
//! picker's `.locale(..)`). This module covers the *fixed* strings the
//! components draw or announce themselves: the Select placeholder, the
//! calendar's Previous/Next buttons, the CloseButton's name, the NumberField
//! stepper names, the date and time segment names, the colour channel
//! names, and so on.
//!
//! The application picks one locale with [`set_locale`]; every string then
//! resolves through [`ui_string`] as: an application override
//! ([`set_ui_string`]) → the built-in table for the locale (exact tag, then
//! its language) → en-US. Without any call, every component renders the same
//! en-US text it always has.
//!
//! The built-in translations are copied from the dictionaries HeroUI v3.2.6
//! pins, which is where HeroUI's own strings come from: `react-aria` 3.52.1
//! `dist/private/intl/<package>/<locale>.mjs`, `react-stately` 3.50.0
//! `dist/private/intl/color/<locale>.mjs` (the colour channel names) and
//! `react-aria-components` 1.21.1 `dist/private/intl/<locale>.mjs`. Six
//! strings HeroUI hard-codes in English with no dictionary entry
//! ([`UiString::NoResults`], [`UiString::Loading`], [`UiString::Search`],
//! [`UiString::ClearSelection`], [`UiString::Pagination`],
//! [`UiString::LoadingMore`]) carry HeroGPUI's own translations; reword any
//! of them with [`set_ui_string`].
//!
//! A few strings are templates with one `{..}` placeholder, as in React
//! Aria ([`UiString::Increase`] is `"Increase {fieldLabel}"`, German
//! `"{fieldLabel} erhöhen"`); [`ui_string_with`] fills it.
//!
//! ```
//! use herogpui_components::i18n::{self, UiString};
//!
//! assert_eq!(i18n::lookup("de-DE", UiString::Next), "Weiter");
//! // A region the table lacks falls back to its language...
//! assert_eq!(i18n::lookup("de-AT", UiString::Next), "Weiter");
//! // ...and an unknown language to en-US.
//! assert_eq!(i18n::lookup("xx", UiString::Next), "Next");
//! assert_eq!(i18n::lookup("ja", UiString::Close), "閉じる");
//! assert_eq!(i18n::fill(i18n::lookup("de", UiString::Increase), "Menge"), "Menge erhöhen");
//! ```

use std::collections::HashMap;

use gpui::{App, Global, SharedString};

/// A component chrome string. See the [module docs](self).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum UiString {
    /// Select / Autocomplete placeholder (`selectPlaceholder`, RAC).
    SelectPlaceholder,
    /// Autocomplete's empty-collection message (HeroUI; HeroGPUI translations).
    NoResults,
    /// CloseButton's accessible name (`close`, React Aria toast).
    Close,
    /// Spinner's accessible name (HeroUI; HeroGPUI translations).
    Loading,
    /// Calendar previous-page button (`previous`, React Aria calendar).
    Previous,
    /// Calendar next-page button (`next`, React Aria calendar).
    Next,
    /// Tag remove button (`removeButtonLabel`, React Aria tag).
    Remove,
    /// Breadcrumbs list name (`breadcrumbs`, React Aria breadcrumbs).
    Breadcrumbs,
    /// SearchField placeholder (HeroUI; HeroGPUI translations).
    Search,
    /// NumberField / TimeField increment stepper, a `{fieldLabel}` template
    /// (`increase`, React Aria numberfield).
    Increase,
    /// NumberField / TimeField decrement stepper, a `{fieldLabel}` template
    /// (`decrease`, React Aria numberfield).
    Decrease,
    /// The year segment's name (`year`, React Aria datepicker).
    Year,
    /// The month segment's name (`month`, React Aria datepicker).
    Month,
    /// The day segment's name (`day`, React Aria datepicker).
    Day,
    /// The hour segment's name (`hour`, React Aria datepicker).
    Hour,
    /// The minute segment's name (`minute`, React Aria datepicker).
    Minute,
    /// The second segment's name (`second`, React Aria datepicker).
    Second,
    /// The AM/PM segment's name (`dayPeriod`, React Aria datepicker).
    DayPeriod,
    /// The DatePicker calendar button (`calendar`, React Aria datepicker).
    Calendar,
    /// A selected calendar day, a `{date}` template (`dateSelected`, React
    /// Aria calendar).
    DateSelected,
    /// The hue channel (`hue`, React Stately color).
    Hue,
    /// The saturation channel (`saturation`, React Stately color).
    Saturation,
    /// The HSL lightness channel (`lightness`, React Stately color).
    Lightness,
    /// The HSB brightness channel (`brightness`, React Stately color).
    Brightness,
    /// The red channel (`red`, React Stately color).
    Red,
    /// The green channel (`green`, React Stately color).
    Green,
    /// The blue channel (`blue`, React Stately color).
    Blue,
    /// The alpha channel (`alpha`, React Stately color).
    Alpha,
    /// Autocomplete's clear button (HeroUI; HeroGPUI translations).
    ClearSelection,
    /// Pagination's navigation name (HeroUI; HeroGPUI translations).
    Pagination,
    /// Table's load-more row text while `is_pending` (HeroUI; HeroGPUI
    /// translations: [`UiString::Loading`] with an ellipsis).
    LoadingMore,
    /// ComboBox trigger button (`buttonLabel`, React Aria combobox).
    ShowSuggestions,
    /// Table row selection checkbox (`select`, React Aria table).
    Select,
    /// Table header select-all checkbox (`selectAll`, React Aria table).
    SelectAll,
}

impl UiString {
    /// The number of keys.
    pub const COUNT: usize = 34;

    /// Every key, in declaration order.
    pub const ALL: [UiString; UiString::COUNT] = [
        UiString::SelectPlaceholder,
        UiString::NoResults,
        UiString::Close,
        UiString::Loading,
        UiString::Previous,
        UiString::Next,
        UiString::Remove,
        UiString::Breadcrumbs,
        UiString::Search,
        UiString::Increase,
        UiString::Decrease,
        UiString::Year,
        UiString::Month,
        UiString::Day,
        UiString::Hour,
        UiString::Minute,
        UiString::Second,
        UiString::DayPeriod,
        UiString::Calendar,
        UiString::DateSelected,
        UiString::Hue,
        UiString::Saturation,
        UiString::Lightness,
        UiString::Brightness,
        UiString::Red,
        UiString::Green,
        UiString::Blue,
        UiString::Alpha,
        UiString::ClearSelection,
        UiString::Pagination,
        UiString::LoadingMore,
        UiString::ShowSuggestions,
        UiString::Select,
        UiString::SelectAll,
    ];

    fn index(self) -> usize {
        self as usize
    }
}

/// The locales with built-in translations; en-US first.
pub const LOCALES: [&str; 13] = [
    "en-US", "de-DE", "es-ES", "fr-FR", "it-IT", "nl-NL", "pl-PL", "pt-BR", "sv-SE", "ja-JP",
    "zh-CN", "ko-KR", "ru-RU",
];

const EN: [&str; UiString::COUNT] = [
    "Select an item",        // SelectPlaceholder
    "No results found",      // NoResults
    "Close",                 // Close
    "Loading",               // Loading
    "Previous",              // Previous
    "Next",                  // Next
    "Remove",                // Remove
    "Breadcrumbs",           // Breadcrumbs
    "Search",                // Search
    "Increase {fieldLabel}", // Increase
    "Decrease {fieldLabel}", // Decrease
    "year",                  // Year
    "month",                 // Month
    "day",                   // Day
    "hour",                  // Hour
    "minute",                // Minute
    "second",                // Second
    "AM/PM",                 // DayPeriod
    "Calendar",              // Calendar
    "{date} selected",       // DateSelected
    "Hue",                   // Hue
    "Saturation",            // Saturation
    "Lightness",             // Lightness
    "Brightness",            // Brightness
    "Red",                   // Red
    "Green",                 // Green
    "Blue",                  // Blue
    "Alpha",                 // Alpha
    "Clear selection",       // ClearSelection
    "pagination",            // Pagination
    "Loading\u{2026}",       // LoadingMore
    "Show suggestions",      // ShowSuggestions
    "Select",                // Select
    "Select All",            // SelectAll
];

const TABLE: [[&str; UiString::COUNT]; 12] = [
    // de-DE
    [
        "Element wählen",            // SelectPlaceholder
        "Keine Ergebnisse gefunden", // NoResults
        "Schließen",                 // Close
        "Wird geladen",              // Loading
        "Zurück",                    // Previous
        "Weiter",                    // Next
        "Entfernen",                 // Remove
        "Breadcrumbs",               // Breadcrumbs
        "Suchen",                    // Search
        "{fieldLabel} erhöhen",      // Increase
        "{fieldLabel} verringern",   // Decrease
        "Jahr",                      // Year
        "Monat",                     // Month
        "Tag",                       // Day
        "Stunde",                    // Hour
        "Minute",                    // Minute
        "Sekunde",                   // Second
        "Tageshälfte",               // DayPeriod
        "Kalender",                  // Calendar
        "{date} ausgewählt",         // DateSelected
        "Farbton",                   // Hue
        "Sättigung",                 // Saturation
        "Leuchtkraft",               // Lightness
        "Helligkeit",                // Brightness
        "Rot",                       // Red
        "Grün",                      // Green
        "Blau",                      // Blue
        "Alpha",                     // Alpha
        "Auswahl löschen",           // ClearSelection
        "Seitennummerierung",        // Pagination
        "Wird geladen…",             // LoadingMore
        "Empfehlungen anzeigen",     // ShowSuggestions
        "Auswählen",                 // Select
        "Alles auswählen",           // SelectAll
    ],
    // es-ES
    [
        "Seleccionar un artículo",      // SelectPlaceholder
        "No se encontraron resultados", // NoResults
        "Cerrar",                       // Close
        "Cargando",                     // Loading
        "Anterior",                     // Previous
        "Siguiente",                    // Next
        "Quitar",                       // Remove
        "Migas de pan",                 // Breadcrumbs
        "Buscar",                       // Search
        "Aumentar {fieldLabel}",        // Increase
        "Reducir {fieldLabel}",         // Decrease
        "año",                          // Year
        "mes",                          // Month
        "día",                          // Day
        "hora",                         // Hour
        "minuto",                       // Minute
        "segundo",                      // Second
        "a.\u{a0}m./p.\u{a0}m.",        // DayPeriod
        "Calendario",                   // Calendar
        "{date} seleccionado",          // DateSelected
        "Tono",                         // Hue
        "Saturación",                   // Saturation
        "Luminosidad",                  // Lightness
        "Brillo",                       // Brightness
        "Rojo",                         // Red
        "Verde",                        // Green
        "Azul",                         // Blue
        "Alpha",                        // Alpha
        "Borrar selección",             // ClearSelection
        "paginación",                   // Pagination
        "Cargando…",                    // LoadingMore
        "Mostrar sugerencias",          // ShowSuggestions
        "Seleccionar",                  // Select
        "Seleccionar todos",            // SelectAll
    ],
    // fr-FR
    [
        "Sélectionner un élément",  // SelectPlaceholder
        "Aucun résultat trouvé",    // NoResults
        "Fermer",                   // Close
        "Chargement",               // Loading
        "Précédent",                // Previous
        "Suivant",                  // Next
        "Supprimer",                // Remove
        "Chemin de navigation",     // Breadcrumbs
        "Rechercher",               // Search
        "Augmenter {fieldLabel}",   // Increase
        "Diminuer {fieldLabel}",    // Decrease
        "année",                    // Year
        "mois",                     // Month
        "jour",                     // Day
        "heure",                    // Hour
        "minute",                   // Minute
        "seconde",                  // Second
        "cadran",                   // DayPeriod
        "Calendrier",               // Calendar
        "{date} sélectionné",       // DateSelected
        "Teinte",                   // Hue
        "Saturation",               // Saturation
        "Luminosité",               // Lightness
        "Luminosité",               // Brightness
        "Rouge",                    // Red
        "Vert",                     // Green
        "Bleu",                     // Blue
        "Alpha",                    // Alpha
        "Effacer la sélection",     // ClearSelection
        "pagination",               // Pagination
        "Chargement…",              // LoadingMore
        "Afficher les suggestions", // ShowSuggestions
        "Sélectionner",             // Select
        "Sélectionner tout",        // SelectAll
    ],
    // it-IT
    [
        "Seleziona un elemento",    // SelectPlaceholder
        "Nessun risultato trovato", // NoResults
        "Chiudi",                   // Close
        "Caricamento",              // Loading
        "Precedente",               // Previous
        "Successivo",               // Next
        "Rimuovi",                  // Remove
        "Breadcrumb",               // Breadcrumbs
        "Cerca",                    // Search
        "Aumenta {fieldLabel}",     // Increase
        "Riduci {fieldLabel}",      // Decrease
        "anno",                     // Year
        "mese",                     // Month
        "giorno",                   // Day
        "ora",                      // Hour
        "minuto",                   // Minute
        "secondo",                  // Second
        "AM/PM",                    // DayPeriod
        "Calendario",               // Calendar
        "{date} selezionata",       // DateSelected
        "Tonalità",                 // Hue
        "Saturazione",              // Saturation
        "Luminosità",               // Lightness
        "Luminosità",               // Brightness
        "Rosso",                    // Red
        "Verde",                    // Green
        "Blu",                      // Blue
        "Alfa",                     // Alpha
        "Cancella selezione",       // ClearSelection
        "paginazione",              // Pagination
        "Caricamento…",             // LoadingMore
        "Mostra suggerimenti",      // ShowSuggestions
        "Seleziona",                // Select
        "Seleziona tutto",          // SelectAll
    ],
    // nl-NL
    [
        "Selecteer een item",       // SelectPlaceholder
        "Geen resultaten gevonden", // NoResults
        "Sluiten",                  // Close
        "Laden",                    // Loading
        "Vorige",                   // Previous
        "Volgende",                 // Next
        "Verwijderen",              // Remove
        "Broodkruimels",            // Breadcrumbs
        "Zoeken",                   // Search
        "{fieldLabel} verhogen",    // Increase
        "{fieldLabel} verlagen",    // Decrease
        "jaar",                     // Year
        "maand",                    // Month
        "dag",                      // Day
        "uur",                      // Hour
        "minuut",                   // Minute
        "seconde",                  // Second
        "a.m./p.m.",                // DayPeriod
        "Kalender",                 // Calendar
        "{date} geselecteerd",      // DateSelected
        "Kleurtoon",                // Hue
        "Verzadiging",              // Saturation
        "Lichtsterkte",             // Lightness
        "Helderheid",               // Brightness
        "Rood",                     // Red
        "Groen",                    // Green
        "Blauw",                    // Blue
        "Alfa",                     // Alpha
        "Selectie wissen",          // ClearSelection
        "paginering",               // Pagination
        "Laden…",                   // LoadingMore
        "Suggesties weergeven",     // ShowSuggestions
        "Selecteren",               // Select
        "Alles selecteren",         // SelectAll
    ],
    // pl-PL
    [
        "Wybierz element",                // SelectPlaceholder
        "Nie znaleziono wyników",         // NoResults
        "Zamknij",                        // Close
        "Ładowanie",                      // Loading
        "Wstecz",                         // Previous
        "Dalej",                          // Next
        "Usuń",                           // Remove
        "Struktura nawigacyjna",          // Breadcrumbs
        "Szukaj",                         // Search
        "Zwiększ {fieldLabel}",           // Increase
        "Zmniejsz {fieldLabel}",          // Decrease
        "rok",                            // Year
        "miesiąc",                        // Month
        "dzień",                          // Day
        "godzina",                        // Hour
        "minuta",                         // Minute
        "sekunda",                        // Second
        "rano / po południu / wieczorem", // DayPeriod
        "Kalendarz",                      // Calendar
        "Wybrano {date}",                 // DateSelected
        "Odcień",                         // Hue
        "Nasycenie",                      // Saturation
        "Jaskrawość",                     // Lightness
        "Jasność",                        // Brightness
        "Czerwony",                       // Red
        "Zielony",                        // Green
        "Niebieski",                      // Blue
        "Alfa",                           // Alpha
        "Wyczyść zaznaczenie",            // ClearSelection
        "paginacja",                      // Pagination
        "Ładowanie…",                     // LoadingMore
        "Wyświetlaj sugestie",            // ShowSuggestions
        "Zaznacz",                        // Select
        "Zaznacz wszystko",               // SelectAll
    ],
    // pt-BR
    [
        "Selecione um item",           // SelectPlaceholder
        "Nenhum resultado encontrado", // NoResults
        "Fechar",                      // Close
        "Carregando",                  // Loading
        "Anterior",                    // Previous
        "Próximo",                     // Next
        "Remover",                     // Remove
        "Caminho detalhado",           // Breadcrumbs
        "Pesquisar",                   // Search
        "Aumentar {fieldLabel}",       // Increase
        "Diminuir {fieldLabel}",       // Decrease
        "ano",                         // Year
        "mês",                         // Month
        "dia",                         // Day
        "hora",                        // Hour
        "minuto",                      // Minute
        "segundo",                     // Second
        "AM/PM",                       // DayPeriod
        "Calendário",                  // Calendar
        "{date} selecionado",          // DateSelected
        "Matiz",                       // Hue
        "Saturação",                   // Saturation
        "Luminosidade",                // Lightness
        "Brilho",                      // Brightness
        "Vermelho",                    // Red
        "Verde",                       // Green
        "Azul",                        // Blue
        "Alfa",                        // Alpha
        "Limpar seleção",              // ClearSelection
        "paginação",                   // Pagination
        "Carregando…",                 // LoadingMore
        "Mostrar sugestões",           // ShowSuggestions
        "Selecionar",                  // Select
        "Selecionar tudo",             // SelectAll
    ],
    // sv-SE
    [
        "Välj en artikel",        // SelectPlaceholder
        "Inga resultat hittades", // NoResults
        "Stäng",                  // Close
        "Läser in",               // Loading
        "Föregående",             // Previous
        "Nästa",                  // Next
        "Ta bort",                // Remove
        "Sökvägar",               // Breadcrumbs
        "Sök",                    // Search
        "Öka {fieldLabel}",       // Increase
        "Minska {fieldLabel}",    // Decrease
        "år",                     // Year
        "månad",                  // Month
        "dag",                    // Day
        "timme",                  // Hour
        "minut",                  // Minute
        "sekund",                 // Second
        "fm/em",                  // DayPeriod
        "Kalender",               // Calendar
        "{date} har valts",       // DateSelected
        "Nyans",                  // Hue
        "Mättnad",                // Saturation
        "Ljushet",                // Lightness
        "Ljusstyrka",             // Brightness
        "Rött",                   // Red
        "Grönt",                  // Green
        "Blått",                  // Blue
        "Alfa",                   // Alpha
        "Rensa urval",            // ClearSelection
        "sidnumrering",           // Pagination
        "Läser in…",              // LoadingMore
        "Visa förslag",           // ShowSuggestions
        "Markera",                // Select
        "Markera allt",           // SelectAll
    ],
    // ja-JP
    [
        "項目を選択",           // SelectPlaceholder
        "結果が見つかりません", // NoResults
        "閉じる",               // Close
        "読み込み中",           // Loading
        "前へ",                 // Previous
        "次へ",                 // Next
        "削除",                 // Remove
        "パンくずリスト",       // Breadcrumbs
        "検索",                 // Search
        "{fieldLabel}を拡大",   // Increase
        "{fieldLabel}を縮小",   // Decrease
        "年",                   // Year
        "月",                   // Month
        "日",                   // Day
        "時",                   // Hour
        "分",                   // Minute
        "秒",                   // Second
        "午前/午後",            // DayPeriod
        "カレンダー",           // Calendar
        "{date} を選択",        // DateSelected
        "色相",                 // Hue
        "彩度",                 // Saturation
        "明度",                 // Lightness
        "明るさ",               // Brightness
        "赤",                   // Red
        "緑",                   // Green
        "青",                   // Blue
        "アルファ",             // Alpha
        "選択をクリア",         // ClearSelection
        "ページネーション",     // Pagination
        "読み込み中…",          // LoadingMore
        "候補を表示",           // ShowSuggestions
        "選択",                 // Select
        "すべて選択",           // SelectAll
    ],
    // zh-CN
    [
        "选择一个项目",      // SelectPlaceholder
        "未找到结果",        // NoResults
        "关闭",              // Close
        "加载中",            // Loading
        "上一页",            // Previous
        "下一页",            // Next
        "删除",              // Remove
        "导航栏",            // Breadcrumbs
        "搜索",              // Search
        "提高 {fieldLabel}", // Increase
        "降低 {fieldLabel}", // Decrease
        "年",                // Year
        "月",                // Month
        "日",                // Day
        "小时",              // Hour
        "分钟",              // Minute
        "秒",                // Second
        "上午/下午",         // DayPeriod
        "日历",              // Calendar
        "已选择 {date}",     // DateSelected
        "色相",              // Hue
        "饱和度",            // Saturation
        "明亮度",            // Lightness
        "亮度",              // Brightness
        "红色",              // Red
        "绿色",              // Green
        "蓝色",              // Blue
        "Alpha",             // Alpha
        "清除选择",          // ClearSelection
        "分页",              // Pagination
        "加载中…",           // LoadingMore
        "显示建议",          // ShowSuggestions
        "选择",              // Select
        "全选",              // SelectAll
    ],
    // ko-KR
    [
        "항목 선택",               // SelectPlaceholder
        "결과를 찾을 수 없습니다", // NoResults
        "닫기",                    // Close
        "로드 중",                 // Loading
        "이전",                    // Previous
        "다음",                    // Next
        "제거",                    // Remove
        "탐색 표시",               // Breadcrumbs
        "검색",                    // Search
        "{fieldLabel} 증가",       // Increase
        "{fieldLabel} 감소",       // Decrease
        "년",                      // Year
        "월",                      // Month
        "일",                      // Day
        "시",                      // Hour
        "분",                      // Minute
        "초",                      // Second
        "오전/오후",               // DayPeriod
        "달력",                    // Calendar
        "{date} 선택됨",           // DateSelected
        "색조",                    // Hue
        "채도",                    // Saturation
        "밝기",                    // Lightness
        "명도",                    // Brightness
        "빨강",                    // Red
        "초록",                    // Green
        "파랑",                    // Blue
        "알파",                    // Alpha
        "선택 지우기",             // ClearSelection
        "페이지 매김",             // Pagination
        "로드 중…",                // LoadingMore
        "제안 사항 표시",          // ShowSuggestions
        "선택",                    // Select
        "모두 선택",               // SelectAll
    ],
    // ru-RU
    [
        "Выберите элемент",        // SelectPlaceholder
        "Ничего не найдено",       // NoResults
        "Закрыть",                 // Close
        "Загрузка",                // Loading
        "Назад",                   // Previous
        "Далее",                   // Next
        "Удалить",                 // Remove
        "Навигация",               // Breadcrumbs
        "Поиск",                   // Search
        "Увеличение {fieldLabel}", // Increase
        "Уменьшение {fieldLabel}", // Decrease
        "год",                     // Year
        "месяц",                   // Month
        "день",                    // Day
        "час",                     // Hour
        "минута",                  // Minute
        "секунда",                 // Second
        "AM/PM",                   // DayPeriod
        "Календарь",               // Calendar
        "Выбрано {date}",          // DateSelected
        "Оттенок",                 // Hue
        "Насыщенность",            // Saturation
        "Освещенность",            // Lightness
        "Яркость",                 // Brightness
        "Красный",                 // Red
        "Зеленый",                 // Green
        "Синий",                   // Blue
        "Альфа",                   // Alpha
        "Очистить выбор",          // ClearSelection
        "нумерация страниц",       // Pagination
        "Загрузка…",               // LoadingMore
        "Показать предложения",    // ShowSuggestions
        "Выбрать",                 // Select
        "Выбрать все",             // SelectAll
    ],
];

fn language(tag: &str) -> &str {
    tag.split(['-', '_']).next().unwrap_or(tag)
}

/// Resolves `tag` to one of [`LOCALES`]: the exact tag (case-insensitive,
/// `_` accepted for `-`), else the first built-in locale with the same
/// language, else `"en-US"`.
pub fn resolve_locale(tag: &str) -> &'static str {
    let normalized = tag.replace('_', "-");
    if let Some(exact) = LOCALES.iter().find(|l| l.eq_ignore_ascii_case(&normalized)) {
        return exact;
    }
    let lang = language(&normalized);
    LOCALES
        .iter()
        .find(|l| language(l).eq_ignore_ascii_case(lang))
        .copied()
        .unwrap_or("en-US")
}

/// The built-in string for `key` in `locale`, with the fallbacks
/// [`resolve_locale`] describes; en-US where the table has no entry.
/// Ignores application overrides; see [`ui_string`] for those.
pub fn lookup(locale: &str, key: UiString) -> &'static str {
    let resolved = resolve_locale(locale);
    LOCALES
        .iter()
        .position(|l| *l == resolved)
        .and_then(|row| row.checked_sub(1))
        .map_or(EN[key.index()], |row| TABLE[row][key.index()])
}

/// Fills a template's one `{..}` placeholder with `arg`, trimming the
/// space an empty `arg` leaves (`"Increase {fieldLabel}"` with no label is
/// `"Increase"`). A string without a placeholder comes back unchanged.
pub fn fill(template: &str, arg: &str) -> String {
    match template
        .find('{')
        .and_then(|open| Some((open, open + template[open..].find('}')?)))
    {
        Some((open, close)) => {
            let filled = format!("{}{arg}{}", &template[..open], &template[close + 1..]);
            filled.trim().to_owned()
        }
        None => template.to_owned(),
    }
}

/// The application's string settings: the active locale and any overrides.
/// A GPUI global; absent means en-US with no overrides.
#[derive(Default)]
struct UiStrings {
    locale: Option<SharedString>,
    overrides: HashMap<(SharedString, UiString), SharedString>,
}

impl Global for UiStrings {}

/// Sets the locale component chrome strings resolve in, and repaints.
/// Accepts any BCP 47 tag; see [`resolve_locale`] for the fallback.
pub fn set_locale(locale: impl Into<SharedString>, cx: &mut App) {
    cx.default_global::<UiStrings>().locale = Some(locale.into());
    cx.refresh_windows();
}

/// The locale set with [`set_locale`], or `"en-US"`.
pub fn locale(cx: &App) -> SharedString {
    cx.try_global::<UiStrings>()
        .and_then(|s| s.locale.clone())
        .unwrap_or_else(|| SharedString::new_static("en-US"))
}

/// Direction inherited by positioned controls, matching React Aria's script-first fallback.
pub(crate) fn is_rtl(tag: &str) -> bool {
    let Ok(mut locale) = tag.replace('_', "-").parse::<icu_locale_core::Locale>() else {
        return false;
    };
    icu_locale::LocaleExpander::new_extended().maximize(&mut locale.id);
    if let Some(script) = locale.id.script {
        return matches!(
            script.as_str(),
            "Arab" | "Syrc" | "Samr" | "Mand" | "Thaa" | "Mend" | "Nkoo" | "Adlm" | "Rohg" | "Hebr"
        );
    }
    matches!(
        locale.id.language.as_str(),
        "ae" | "ar"
            | "arc"
            | "bcc"
            | "bqi"
            | "ckb"
            | "dv"
            | "fa"
            | "glk"
            | "he"
            | "ku"
            | "mzn"
            | "nqo"
            | "pnb"
            | "ps"
            | "sd"
            | "ug"
            | "ur"
            | "yi"
    )
}

/// Overrides `key` for `locale`. An override keyed by a full tag (`"fr-CA"`)
/// applies to that tag only; one keyed by a bare language (`"fr"`) applies to
/// every locale of that language without an exact override. Use it to translate a string the built-in table
/// lacks, or to reword one.
pub fn set_ui_string(
    locale: impl Into<SharedString>,
    key: UiString,
    value: impl Into<SharedString>,
    cx: &mut App,
) {
    cx.default_global::<UiStrings>()
        .overrides
        .insert((locale.into(), key), value.into());
    cx.refresh_windows();
}

/// The string for `key` in the active locale: an override, else the
/// built-in table, else en-US.
pub fn ui_string(key: UiString, cx: &App) -> SharedString {
    let Some(strings) = cx.try_global::<UiStrings>() else {
        return SharedString::new_static(EN[key.index()]);
    };
    let active = strings
        .locale
        .clone()
        .unwrap_or_else(|| SharedString::new_static("en-US"));
    let lang = language(&active);
    let found = strings
        .overrides
        .get(&(active.clone(), key))
        .or_else(|| {
            strings
                .overrides
                .get(&(SharedString::from(lang.to_owned()), key))
        })
        .cloned();
    found.unwrap_or_else(|| SharedString::new_static(lookup(&active, key)))
}

/// [`ui_string`] with its `{..}` placeholder filled with `arg` (see
/// [`fill`]); the same as [`ui_string`] for a key that is not a template.
pub fn ui_string_with(key: UiString, arg: &str, cx: &App) -> SharedString {
    fill(&ui_string(key, cx), arg).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn en_us_matches_the_strings_components_drew_before() {
        for key in UiString::ALL {
            assert_eq!(lookup("en-US", key), EN[key.index()]);
        }
        assert_eq!(
            lookup("en-US", UiString::SelectPlaceholder),
            "Select an item"
        );
    }

    #[test]
    fn every_locale_resolves_every_key() {
        for locale in LOCALES {
            for key in UiString::ALL {
                assert!(!lookup(locale, key).is_empty(), "{locale} {key:?}");
            }
        }
    }

    #[test]
    fn fallback_chain() {
        assert_eq!(resolve_locale("fr_CA"), "fr-FR");
        assert_eq!(resolve_locale("PT-pt"), "pt-BR");
        assert_eq!(resolve_locale("ja"), "ja-JP");
        assert_eq!(resolve_locale("zh-Hans-CN"), "zh-CN");
        assert_eq!(resolve_locale("tr-TR"), "en-US");
        assert_eq!(lookup("sv-SE", UiString::Previous), "Föregående");
    }

    /// The new locales carry every key in their own script, spot-checked
    /// against the pinned React Aria / React Stately dictionaries.
    #[test]
    fn the_non_latin_locales_are_translated() {
        assert_eq!(lookup("ja-JP", UiString::SelectPlaceholder), "項目を選択");
        assert_eq!(lookup("zh-CN", UiString::Next), "下一页");
        assert_eq!(lookup("ko-KR", UiString::Close), "닫기");
        assert_eq!(lookup("ru-RU", UiString::Hue), "Оттенок");
        for locale in ["ja-JP", "zh-CN", "ko-KR", "ru-RU"] {
            for key in UiString::ALL {
                // The dictionaries keep these two in Latin script.
                if matches!(
                    (locale, key),
                    ("zh-CN", UiString::Alpha) | ("ru-RU", UiString::DayPeriod)
                ) {
                    continue;
                }
                assert_ne!(
                    lookup(locale, key),
                    lookup("en-US", key),
                    "{locale} {key:?}"
                );
            }
        }
    }

    /// Table's pending row reads the Spinner's word with an ellipsis in every
    /// locale, so the two HeroGPUI-owned strings cannot drift apart.
    #[test]
    fn loading_more_is_the_loading_word_with_an_ellipsis() {
        for locale in LOCALES {
            assert_eq!(
                lookup(locale, UiString::LoadingMore),
                format!("{}\u{2026}", lookup(locale, UiString::Loading)),
                "{locale}"
            );
        }
        assert_eq!(lookup("en-US", UiString::LoadingMore), "Loading\u{2026}");
    }

    #[test]
    fn templates_fill_their_placeholder_in_the_locale_word_order() {
        assert_eq!(
            fill(lookup("en-US", UiString::Increase), "Quantity"),
            "Increase Quantity"
        );
        assert_eq!(fill(lookup("en-US", UiString::Decrease), ""), "Decrease");
        assert_eq!(
            fill(lookup("de-DE", UiString::Decrease), "Menge"),
            "Menge verringern"
        );
        assert_eq!(fill(lookup("ja-JP", UiString::Increase), ""), "を拡大");
        assert_eq!(
            fill(lookup("pl-PL", UiString::DateSelected), "1 maja"),
            "Wybrano 1 maja"
        );
        assert_eq!(fill("Close", "ignored"), "Close");
        // Exactly the keys React Aria formats with an argument are templates.
        for key in UiString::ALL {
            let template = matches!(
                key,
                UiString::Increase | UiString::Decrease | UiString::DateSelected
            );
            for locale in LOCALES {
                assert_eq!(
                    lookup(locale, key).contains('{'),
                    template,
                    "{locale} {key:?}"
                );
            }
        }
    }
}

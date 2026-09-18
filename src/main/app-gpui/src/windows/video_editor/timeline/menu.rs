use gpui::{SharedString, WeakEntity};
use herogpui::gpui;

use crate::ui::menu::{MenuBuilder, MenuEntry, MenuItem};
use crate::windows::video_editor::timeline::tracks::{Clip, MusicLane, TrackKind};
use crate::windows::video_editor::timeline::{format_speed, format_zoom_level, SPEED_PRESETS};
use crate::windows::video_editor::VideoEditorWindow;

pub const ZOOM_LEVELS: [f64; 8] = [1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 2.75, 3.0];

pub fn has_menu(kind: TrackKind) -> bool {
    kind != TrackKind::Video
}

pub fn clip_menu(
    kind: TrackKind,
    clip: &Clip,
    music: Option<&MusicLane>,
    siblings: usize,
    editor: WeakEntity<VideoEditorWindow>,
) -> Vec<MenuEntry> {
    match kind {
        TrackKind::Video => Vec::new(),
        TrackKind::Zoom => zoom_menu(clip, siblings, editor),
        TrackKind::Camera | TrackKind::Drawing => MenuBuilder::new()
            .item(delete(kind, &clip.id, editor))
            .build(),
        TrackKind::Music => music_menu(music, editor),
    }
}

fn delete(kind: TrackKind, id: &SharedString, editor: WeakEntity<VideoEditorWindow>) -> MenuItem {
    let id = id.clone();
    MenuItem::new("Delete")
        .icon("trash-2")
        .danger()
        .on_select(move |_window, cx| {
            if let Some(editor) = editor.upgrade() {
                editor.update(cx, |this, cx| this.delete_clip(kind, id.clone(), cx));
            }
        })
}

fn zoom_menu(
    clip: &Clip,
    siblings: usize,
    editor: WeakEntity<VideoEditorWindow>,
) -> Vec<MenuEntry> {
    let current = clip.zoom_level.unwrap_or_default();
    let mut levels = MenuBuilder::new();
    for value in ZOOM_LEVELS {
        let editor = editor.clone();
        let id = clip.id.clone();
        levels = levels.item(
            MenuItem::new(format_zoom_level(value))
                .radio((current - value).abs() < f64::EPSILON)
                .on_select(move |_window, cx| {
                    if let Some(editor) = editor.upgrade() {
                        editor.update(cx, |this, cx| {
                            this.set_zoom_segment_level(id.clone(), value, cx)
                        });
                    }
                }),
        );
    }

    MenuBuilder::new()
        .item(
            MenuItem::new("Zoom Level")
                .icon("zoom-in")
                .submenu(levels.build()),
        )
        .separator()
        .item({
            let editor = editor.clone();
            let id = clip.id.clone();
            MenuItem::new("Apply zoom to All")
                .icon("copy")
                .disabled(siblings <= 1)
                .on_select(move |_window, cx| {
                    if let Some(editor) = editor.upgrade() {
                        editor.update(cx, |this, cx| this.apply_zoom_level_to_all(id.clone(), cx));
                    }
                })
        })
        .separator()
        .item(delete(TrackKind::Zoom, &clip.id, editor.clone()))
        .item({
            let id = clip.id.clone();
            MenuItem::new("Delete Others")
                .icon("trash-2")
                .disabled(siblings <= 1)
                .on_select(move |_window, cx| {
                    if let Some(editor) = editor.upgrade() {
                        editor.update(cx, |this, cx| {
                            this.delete_other_clips(TrackKind::Zoom, id.clone(), cx)
                        });
                    }
                })
        })
        .build()
}

fn music_menu(music: Option<&MusicLane>, editor: WeakEntity<VideoEditorWindow>) -> Vec<MenuEntry> {
    let Some(lane) = music else {
        return Vec::new();
    };

    let mut speeds = MenuBuilder::new();
    for value in SPEED_PRESETS {
        let editor = editor.clone();
        let group = lane.group_id.clone();
        speeds = speeds.item(
            MenuItem::new(format_speed(value))
                .radio((lane.speed - value).abs() < f64::EPSILON)
                .on_select(move |_window, cx| {
                    if let Some(editor) = editor.upgrade() {
                        editor.update(cx, |this, cx| {
                            this.set_music_group_speed(group.clone(), value, cx)
                        });
                    }
                }),
        );
    }

    let mut builder =
        MenuBuilder::new().item(MenuItem::new("Speed").icon("gauge").submenu(speeds.build()));
    if !lane.removable {
        return builder.build();
    }

    let group = lane.group_id.clone();
    builder =
        builder
            .separator()
            .item(MenuItem::new("Remove").icon("trash-2").danger().on_select(
                move |_window, cx| {
                    if let Some(editor) = editor.upgrade() {
                        editor.update(cx, |this, cx| this.remove_music_group(group.clone(), cx));
                    }
                },
            ));
    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(zoom_level: Option<f64>) -> Clip {
        Clip {
            zoom_level,
            ..Clip::new("a".to_string(), false, 0.0, 1.0)
        }
    }

    fn lane(removable: bool) -> MusicLane {
        MusicLane {
            group_id: "g".into(),
            speed: 1.0,
            removable,
        }
    }

    fn labels(entries: &[MenuEntry]) -> Vec<String> {
        entries
            .iter()
            .map(|entry| match entry {
                MenuEntry::Item(item) => item.label.to_string(),
                MenuEntry::Separator => "-".to_string(),
                MenuEntry::Label(label) => label.to_string(),
            })
            .collect()
    }

    #[test]
    fn a_video_clip_has_no_context_menu() {
        assert!(!has_menu(TrackKind::Video));
        assert!(clip_menu(
            TrackKind::Video,
            &clip(None),
            None,
            1,
            WeakEntity::new_invalid()
        )
        .is_empty());
    }

    #[test]
    fn a_camera_or_drawing_clip_offers_delete_only() {
        for kind in [TrackKind::Camera, TrackKind::Drawing] {
            let entries = clip_menu(kind, &clip(None), None, 3, WeakEntity::new_invalid());
            assert_eq!(labels(&entries), vec!["Delete"]);
        }
    }

    #[test]
    fn a_zoom_clip_offers_the_level_submenu_and_the_delete_pair() {
        let entries = clip_menu(
            TrackKind::Zoom,
            &clip(Some(1.5)),
            None,
            1,
            WeakEntity::new_invalid(),
        );
        assert_eq!(
            labels(&entries),
            vec![
                "Zoom Level",
                "-",
                "Apply zoom to All",
                "-",
                "Delete",
                "Delete Others"
            ]
        );
    }

    #[test]
    fn a_music_clip_offers_speed_and_only_removes_an_imported_group() {
        let built_in = clip_menu(
            TrackKind::Music,
            &clip(None),
            Some(&lane(false)),
            1,
            WeakEntity::new_invalid(),
        );
        assert_eq!(labels(&built_in), vec!["Speed"]);

        let imported = clip_menu(
            TrackKind::Music,
            &clip(None),
            Some(&lane(true)),
            1,
            WeakEntity::new_invalid(),
        );
        assert_eq!(labels(&imported), vec!["Speed", "-", "Remove"]);
    }
}

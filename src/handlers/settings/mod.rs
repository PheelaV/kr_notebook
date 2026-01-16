//! Settings handlers for user preferences and admin operations.

mod admin;
mod audio;
mod user;

use std::path::Path as StdPath;

use crate::paths;

// Re-export public items
pub use admin::{
  cleanup_guests, delete_all_guests, delete_scraped, delete_scraped_lesson, graduate_tier,
  make_all_due, restore_tier, trigger_manual_segment, trigger_reset_segment, trigger_row_segment,
  trigger_scrape, trigger_scrape_lesson, trigger_segment, AudioRowTemplate, ManualSegmentForm,
  ResetSegmentForm, RowSegmentForm, SegmentForm,
  // User/group management
  set_user_role, create_group, delete_group, add_to_group, remove_from_group,
  SetRoleForm, CreateGroupForm, GroupMemberForm,
  // Pack permissions (groups and users)
  restrict_pack_to_group, remove_pack_restriction, make_pack_public, make_pack_private, PackPermissionForm,
  restrict_pack_to_user, remove_pack_user_restriction, PackUserPermissionForm,
  // External pack paths (admin)
  register_pack_path, unregister_pack_path, toggle_pack_path, browse_directories,
  RegisterPackPathForm, RegisteredPathDisplay, RegisteredPathsTemplate, render_registered_paths,
  DirectoryBrowserTemplate, DirectoryEntry, BrowseDirectoryForm,
};
pub use audio::{
  get_audio_row, get_lesson_audio, AudioRow, LessonAudio, SegmentParams, SyllablePreview,
  TierGraduationStatus,
};
pub use user::{
  disable_pack, enable_pack, export_data, import_data, settings_page, update_settings, PackInfo,
  SettingsForm, SettingsTemplate, UserDisplay, GroupDisplay,
};

/// Check if lesson content exists for a given lesson ID
pub fn has_lesson(lesson_id: &str) -> bool {
  StdPath::new(&paths::manifest_path(lesson_id)).exists()
}

/// Check if lesson1 content exists
pub fn has_lesson1() -> bool {
  has_lesson("lesson1")
}

/// Check if lesson2 content exists
pub fn has_lesson2() -> bool {
  has_lesson("lesson2")
}

/// Check if lesson3 content exists
pub fn has_lesson3() -> bool {
  has_lesson("lesson3")
}

/// Count segmented syllables for a lesson
pub(crate) fn count_syllables(lesson: &str) -> usize {
  let path = paths::syllables_dir(lesson);
  std::fs::read_dir(path)
    .map(|entries| {
      entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|ext| ext == "mp3").unwrap_or(false))
        .count()
    })
    .unwrap_or(0)
}

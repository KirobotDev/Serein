use local_store::AppPreferences;

#[derive(Default)]
pub struct Settings {
	pub current: AppPreferences,
	pub loaded: bool,
	pub state: crate::toggle_setting::Settings,
}

impl Settings {
	pub fn save(&mut self, cache: Option<&crate::cache::Cache>, generation: u64) -> bool {
		if !self.state.dirty || self.state.saving {
			return false;
		}
		let accepted = cache.is_some_and(|cache| {
			cache.queue(
				generation,
				model::Id(0),
				crate::cache::Operation::SaveAppPreferences(Box::new(self.current.clone())),
			)
		});
		// A full cache queue must not turn a device preference into a session-only change.
		self.state.dirty = !accepted;
		self.state.saving = accepted;
		self.state.failed = !accepted;
		accepted
	}
	pub fn observe(&mut self, ui: &ui::MessagingUi) {
		let value = AppPreferences {
			window_geometry: self.current.window_geometry,
			language: ui.language.preference().map(str::to_owned),
			notifications_enabled: ui.notifications_enabled,
			auto_update: ui.updates.auto_update,
			update_nightly: ui.updates.nightly,
			notification_options: ui.notification_options,
			show_hidden_channels: ui.show_hidden_channels,
			hide_title_bar: ui.hide_title_bar,
			hide_window_decorations: ui.hide_window_decorations,
			gpu_preference: ui.gpu_preference,
			primary_color: ui.primary_color,
			transparency_blur: ui.transparency_blur,
			transparency: ui.transparency,
			blur: ui.blur,
			voice_noise_suppression: ui.voice_processing.effective().suppression
				!= model::voice_settings::NoiseSuppression::Off,
			voice_processing: Some(ui.voice_processing),
			voice_push_to_talk: ui.voice_push_to_talk,
			voice_muted: ui.voice_muted,
			voice_deafened: ui.voice_deafened,
			voice_input: ui.voice_input.clone(),
			voice_output: ui.voice_output.clone(),
			input_percent: ui.voice_gain.input_percent,
			output_percent: ui.voice_gain.output_percent,
			keybinds: ui.keybinds.clone(),
			expanded_folders: ui.expanded_folders.clone(),
			user_volumes: ui.voice_user_volume_overrides(),
			muted_users: ui.voice_user_mutes().to_vec(),
		};
		if value != self.current {
			self.state.touched = true;
			self.state.failed = !value.is_valid();
			if value.is_valid() {
				self.current = value;
				self.state.dirty = true;
			}
		}
	}
	pub fn apply(&self, ui: &mut ui::MessagingUi) {
		let value = &self.current;
		ui.language = ui::i18n::Language::from_preference(value.language.as_deref());
		ui::i18n::set_current(ui.language);
		ui.notifications_enabled = value.notifications_enabled;
		ui.updates.auto_update = value.auto_update;
		ui.updates.nightly = value.update_nightly;
		ui.notification_options = value.notification_options;
		ui.show_hidden_channels = value.show_hidden_channels;
		ui.hide_title_bar = value.hide_title_bar;
		ui.hide_window_decorations = value.hide_window_decorations;
		ui.gpu_preference = value.gpu_preference;
		ui.primary_color = value.primary_color;
		ui.transparency_blur = value.transparency_blur;
		ui.transparency = value.transparency;
		ui.blur = value.blur;
		ui.voice_processing = value.voice_processing.unwrap_or_else(|| {
			model::voice_settings::VoiceProcessing::from_legacy(value.voice_noise_suppression)
		});
		ui.voice_push_to_talk = value.voice_push_to_talk;
		ui.voice_muted = value.voice_muted;
		ui.voice_deafened = value.voice_deafened;
		ui.voice_input.clone_from(&value.voice_input);
		ui.voice_output.clone_from(&value.voice_output);
		ui.voice_gain.input_percent = value.input_percent;
		ui.voice_gain.output_percent = value.output_percent;
		ui.keybinds = value.keybinds.clone();
		ui.expanded_folders.clone_from(&value.expanded_folders);
		ui.set_voice_user_volume_overrides(&value.user_volumes);
		ui.set_voice_user_mutes(&value.muted_users);
	}
}

#[cfg(all(debug_assertions, feature = "demo"))]
pub fn debug_window_geometry_check() {
	use local_store::{LocalStore, WindowGeometry};
	let path = std::env::temp_dir().join(format!(
		"serein-window-geometry-{}.sqlite3",
		std::process::id()
	));
	assert!(!path.exists());
	let store = LocalStore::open(&path).unwrap();
	assert!(store.app_preferences().unwrap().window_geometry.is_none());
	for position in [None, Some([-1920, 80])] {
		let size = winit::dpi::PhysicalSize::new(1800, 1200).to_logical::<u32>(1.5);
		let geometry = WindowGeometry {
			size: [size.width, size.height],
			position,
		};
		let mut settings = Settings::default();
		settings.current.window_geometry = Some(geometry);
		let mut ui = ui::MessagingUi::default();
		settings.apply(&mut ui);
		ui.notifications_enabled = false;
		settings.observe(&ui);
		assert_eq!(settings.current.window_geometry, Some(geometry));
		store.save_app_preferences(&settings.current).unwrap();
		let reopened = LocalStore::open(&path).unwrap();
		assert_eq!(
			reopened.app_preferences().unwrap().window_geometry,
			Some(geometry)
		);
		for invalid in [
			WindowGeometry {
				size: [0, 800],
				position,
			},
			WindowGeometry {
				size: [16385, 800],
				position,
			},
			WindowGeometry {
				size: [1200, 800],
				position: Some([i32::MAX, 0]),
			},
		] {
			settings.current.window_geometry = Some(invalid);
			assert!(store.save_app_preferences(&settings.current).is_err());
		}
		assert_eq!(
			reopened.app_preferences().unwrap().window_geometry,
			Some(geometry)
		);
	}
	drop(store);
	std::fs::remove_file(path).unwrap();
	println!(
		"Offline window geometry check passed: size without position, X11 coordinates, native scale, preference preservation, SQLite reopen and bounds."
	);
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn startup_defaults_do_not_overwrite_pending_saved_preferences() {
		let mut settings = Settings::default();
		let defaults = AppPreferences::default();
		let mut ui = ui::MessagingUi::default();
		ui.notifications_enabled = defaults.notifications_enabled;
		ui.transparency = defaults.transparency;
		ui.blur = defaults.blur;
		settings.observe(&ui);
		assert!(
			!settings.state.touched,
			"startup defaults must not count as a user edit"
		);
		assert!(!settings.state.dirty);

		settings.current.language = Some("cs".into());
		settings.current.notification_options.current_channel = true;
		settings.loaded = true;
		settings.apply(&mut ui);
		settings.observe(&ui);
		assert_eq!(ui.language, ui::i18n::Language::Czech);
		assert!(ui.notification_options.current_channel);
		assert!(!settings.state.touched);
		assert!(!settings.state.dirty);
	}

	#[test]
	fn legacy_preferences_without_voice_settings_keep_suppression_disabled() {
		let current: AppPreferences = serde_json::from_str("{}").unwrap();
		assert!(!current.voice_noise_suppression);
		assert!(current.voice_processing.is_none());
		let settings = Settings {
			current,
			..Default::default()
		};
		let mut ui = ui::MessagingUi::default();
		settings.apply(&mut ui);
		assert_eq!(
			ui.voice_processing.effective().suppression,
			model::voice_settings::NoiseSuppression::Off
		);
	}
}

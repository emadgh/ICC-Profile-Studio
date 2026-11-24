//! Shared updater integration; UI based on windows-image-search/src/ui/update_ui.rs.
use eframe::egui;
use std::time::Duration;
use update_via_github::{UpdateConfig, UpdateManager, UpdateStatus};

pub struct Updates {
    manager: UpdateManager,
    error: Option<String>,
}
impl Default for Updates {
    fn default() -> Self {
        let config = UpdateConfig::new("emadgh/color-convert-rust", "ICC-Profile-Rebuilder.exe", env!("CARGO_PKG_VERSION"))
            .with_app_name("ICC Profile Rebuilder")
            .with_checksum_asset("ICC-Profile-Rebuilder.exe.sha256")
            .with_required_checksum(true);
        Self { manager: UpdateManager::new(config), error: None }
    }
}
impl Updates {
    pub fn tick(&self, ctx: &egui::Context) {
        if matches!(self.manager.status(), UpdateStatus::Checking | UpdateStatus::Downloading { .. }) {
            ctx.request_repaint_after(Duration::from_millis(120));
        }
    }
    pub fn show(&mut self, ui: &mut egui::Ui, busy: bool) {
        let status = self.manager.status();
        match status {
            UpdateStatus::Idle => { ui.label("Check GitHub Releases for a newer version."); }
            UpdateStatus::Checking => { ui.horizontal(|ui| { ui.spinner(); ui.label("Checking releases..."); }); }
            UpdateStatus::UpToDate => {
                // The upstream crate maps HTTP 404 to no update. Never claim a private release was checked.
                ui.label("No newer accessible release found. Private releases may not be visible.");
            }
            UpdateStatus::Available(info) => {
                ui.strong(format!("Version {} is available", info.version));
                ui.hyperlink_to("Release notes", &info.release_url);
                if ui.button("Download update").clicked() { self.manager.start_download(); }
            }
            UpdateStatus::Downloading { info, downloaded, total } => {
                ui.label(format!("Downloading version {}", info.version));
                match total.filter(|n| *n > 0) {
                    Some(n) => { ui.add(egui::ProgressBar::new((downloaded as f32 / n as f32).clamp(0.,1.))
                        .text(format!("{:.1} / {:.1} MiB", downloaded as f64 / 1048576., n as f64 / 1048576.))); }
                    None => { ui.horizontal(|ui| { ui.spinner(); ui.label(format!("{:.1} MiB downloaded", downloaded as f64 / 1048576.)); }); }
                }
            }
            UpdateStatus::Ready(info, _) => {
                ui.strong(format!("Version {} downloaded and verified", info.version));
                if ui.add_enabled(!busy, egui::Button::new("Restart & install")).on_hover_text("Finish active ICC work first").clicked() {
                    match self.manager.apply_ready() {
                        Ok(true) => ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close),
                        Ok(false) => self.error = Some("No staged update found.".into()),
                        Err(e) => self.error = Some(e),
                    }
                }
            }
            UpdateStatus::Failed(e) => { ui.colored_label(ui.visuals().error_fg_color, e); }
        }
        if !matches!(self.manager.status(), UpdateStatus::Checking | UpdateStatus::Downloading { .. } | UpdateStatus::Ready(..)) {
            if ui.button("Check for updates").clicked() { self.error = None; self.manager.start_check(false); }
        }
        if let Some(e) = &self.error { ui.colored_label(ui.visuals().error_fg_color, e); }
        ui.small("Private GitHub repository: the shared updater is unauthenticated. Use Releases while signed in until a public update feed or authenticated updater is configured.");
        ui.hyperlink_to("Open GitHub Releases", "https://github.com/emadgh/color-convert-rust/releases");
    }
}

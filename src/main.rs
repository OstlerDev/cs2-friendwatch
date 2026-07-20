mod config;
mod notify;
mod steam;
mod watcher;

use config::Config;
use eframe::egui::{
    self, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, TextureHandle,
    TextureOptions, Vec2,
};
use notify::{notify_spot_available, open_join, play_alert_sound};
use steam::{FriendInfo, SteamSession};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use watcher::{
    first_joinable, format_elapsed, FriendPresence, JoinMethod, NotifyDebouncer, NotifyKey,
    WatchedFriendStatus,
};

const POLL_INTERVAL: Duration = Duration::from_millis(1500);
const IDLE_REFRESH: Duration = Duration::from_secs(15);
const NOTIFY_COOLDOWN: Duration = Duration::from_secs(30);
const ROW_HEIGHT: f32 = 58.0;
const AVATAR_SIZE: f32 = 40.0;

const BG: Color32 = Color32::from_rgb(14, 16, 18);
const PANEL: Color32 = Color32::from_rgb(22, 26, 30);
const PANEL_ALT: Color32 = Color32::from_rgb(28, 33, 38);
const BORDER: Color32 = Color32::from_rgb(42, 48, 54);
const TEXT: Color32 = Color32::from_rgb(230, 232, 234);
const MUTED: Color32 = Color32::from_rgb(140, 148, 156);
const AMBER: Color32 = Color32::from_rgb(222, 155, 53);
const GREEN: Color32 = Color32::from_rgb(62, 168, 96);
const RED: Color32 = Color32::from_rgb(200, 72, 72);

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([620.0, 720.0])
            .with_min_inner_size([460.0, 520.0])
            .with_title("CS2 Friendwatch"),
        ..Default::default()
    };
    eframe::run_native(
        "CS2 Friendwatch",
        options,
        Box::new(|cc| Ok(Box::new(FriendwatchApp::new(cc)))),
    )
}

#[derive(Clone)]
struct PendingJoin {
    steam_id: u64,
    name: String,
    detail: String,
    method: JoinMethod,
}

struct FriendwatchApp {
    steam: Result<SteamSession, String>,
    steam_app_id: Option<u32>,
    cs2_friends: Vec<FriendInfo>,
    name_cache: HashMap<u64, String>,
    detail_cache: HashMap<u64, String>,
    avatar_tex: HashMap<u64, TextureHandle>,
    watched_order: Vec<u64>,
    watched_set: HashSet<u64>,
    watching: bool,
    /// True after a successful Join until the user starts watching again.
    needs_rewatch: bool,
    /// Fire sound/focus once per pending alert.
    alert_armed: bool,
    watch_started: Option<Instant>,
    last_poll: Option<Instant>,
    last_list_refresh: Option<Instant>,
    statuses: Vec<WatchedFriendStatus>,
    debouncer: NotifyDebouncer,
    pending: Option<PendingJoin>,
    status_msg: String,
    filter: String,
}

impl FriendwatchApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_theme(&cc.egui_ctx);

        let config = Config::load();
        let steam = SteamSession::init();
        let steam_app_id = steam.as_ref().ok().map(|s| s.app_id);
        let mut name_cache = HashMap::new();
        let mut detail_cache = HashMap::new();
        let mut avatar_tex = HashMap::new();

        let cs2_friends = match &steam {
            Ok(s) => {
                s.run_callbacks();
                let list = s.list_cs2_friends();
                ingest_friends(
                    &cc.egui_ctx,
                    &list,
                    &mut name_cache,
                    &mut detail_cache,
                    &mut avatar_tex,
                );
                list
            }
            Err(_) => Vec::new(),
        };

        let watched_order: Vec<u64> = config.watched_steam_ids;
        let watched_set: HashSet<u64> = watched_order.iter().copied().collect();

        let status_msg = match (&steam, steam_app_id) {
            (Ok(_), Some(730)) => format!(
                "Connected as CS2 (730) — {} friend(s) in-game.",
                cs2_friends.len()
            ),
            (Ok(_), Some(480)) => format!(
                "Connected as Spacewar (480) — CS2 was already running. Rich presence may be limited. {} in CS2.",
                cs2_friends.len()
            ),
            (Ok(_), Some(id)) => format!("Connected (app {id}) — {} in CS2.", cs2_friends.len()),
            (Err(e), _) => e.clone(),
            _ => "Connected.".into(),
        };

        Self {
            steam,
            steam_app_id,
            cs2_friends,
            name_cache,
            detail_cache,
            avatar_tex,
            watched_order,
            watched_set,
            watching: false,
            needs_rewatch: false,
            alert_armed: false,
            watch_started: None,
            last_poll: None,
            last_list_refresh: Some(Instant::now()),
            statuses: Vec::new(),
            debouncer: NotifyDebouncer::new(),
            pending: None,
            status_msg,
            filter: String::new(),
        }
    }

    fn persist_watch_list(&self) {
        let cfg = Config {
            watched_steam_ids: self.watched_order.clone(),
        };
        let _ = cfg.save();
    }

    fn toggle_watch(&mut self, steam_id: u64, name: &str, checked: bool) {
        self.name_cache.insert(steam_id, name.to_string());
        if checked {
            if self.watched_set.insert(steam_id) {
                self.watched_order.push(steam_id);
            }
        } else {
            self.watched_set.remove(&steam_id);
            self.watched_order.retain(|id| *id != steam_id);
        }
        self.persist_watch_list();
    }

    fn name_lookup(&self) -> Vec<(u64, String)> {
        self.name_cache.iter().map(|(&id, n)| (id, n.clone())).collect()
    }

    fn refresh_cs2_list(&mut self, ctx: &egui::Context) {
        let Ok(session) = &self.steam else {
            return;
        };
        session.run_callbacks();
        let list = session.list_cs2_friends();
        ingest_friends(
            ctx,
            &list,
            &mut self.name_cache,
            &mut self.detail_cache,
            &mut self.avatar_tex,
        );
        self.cs2_friends = list;
        self.last_list_refresh = Some(Instant::now());
        self.prune_offline_from_watch_queue();
    }

    fn prune_offline_from_watch_queue(&mut self) {
        let in_cs2: HashSet<u64> = self.cs2_friends.iter().map(|f| f.steam_id).collect();
        let status_gone: HashSet<u64> = self
            .statuses
            .iter()
            .filter(|s| {
                matches!(
                    s.presence,
                    FriendPresence::OfflineOrUnknown | FriendPresence::OtherGame { .. }
                )
            })
            .map(|s| s.steam_id)
            .collect();

        let before = self.watched_order.len();
        self.watched_order.retain(|id| {
            if status_gone.contains(id) {
                return false;
            }
            // Not in CS2 list and we have no live "in CS2" status → drop.
            if !in_cs2.contains(id) {
                let still_in_cs2_status = self.statuses.iter().any(|s| {
                    s.steam_id == *id
                        && matches!(
                            s.presence,
                            FriendPresence::InCs2Full | FriendPresence::Joinable { .. }
                        )
                });
                return still_in_cs2_status;
            }
            true
        });
        self.watched_set = self.watched_order.iter().copied().collect();
        if self.watched_order.len() != before {
            self.persist_watch_list();
            if self.watching && self.watched_order.is_empty() {
                self.stop_watching();
                self.status_msg = "Watch queue cleared — no watched friends still in CS2.".into();
            }
        }
    }

    fn poll_once(&mut self, ctx: &egui::Context) {
        let Ok(session) = &self.steam else {
            return;
        };
        session.run_callbacks();
        let names = self.name_lookup();
        let mut statuses = session.poll_watched(&self.watched_order, &names);
        for s in &mut statuses {
            self.name_cache.insert(s.steam_id, s.name.clone());
            if !s.detail.is_empty() {
                self.detail_cache.insert(s.steam_id, s.detail.clone());
            } else if let Some(d) = self.detail_cache.get(&s.steam_id) {
                s.detail = d.clone();
            }
        }
        self.statuses = statuses;
        self.last_poll = Some(Instant::now());

        let list = session.list_cs2_friends();
        ingest_friends(
            ctx,
            &list,
            &mut self.name_cache,
            &mut self.detail_cache,
            &mut self.avatar_tex,
        );
        self.cs2_friends = list;
        self.last_list_refresh = Some(Instant::now());
        self.prune_offline_from_watch_queue();

        if self.pending.is_some() {
            return;
        }

        let joinable = first_joinable(&self.watched_order, &self.statuses).and_then(|s| {
            if let FriendPresence::Joinable { ref method } = s.presence {
                Some((
                    NotifyKey::from_joinable(s.steam_id, method),
                    s.name.clone(),
                    s.detail.clone(),
                    method.clone(),
                ))
            } else {
                None
            }
        });

        let key_only = joinable.as_ref().map(|(k, _, _, _)| k.clone());
        if let Some(key) = self
            .debouncer
            .consider(key_only, Instant::now(), NOTIFY_COOLDOWN)
        {
            let (name, detail, method) = joinable
                .map(|(_, n, d, m)| (n, d, m))
                .unwrap_or_else(|| ("Friend".into(), String::new(), JoinMethod::OpenSlots));
            notify_spot_available(&name);
            play_alert_sound();
            self.pending = Some(PendingJoin {
                steam_id: key.steam_id,
                name,
                detail,
                method,
            });
            self.alert_armed = true;
            self.status_msg = "Spot available — confirm join in the alert window.".into();
        }
    }

    fn start_watching(&mut self) {
        self.watching = true;
        self.needs_rewatch = false;
        self.watch_started = Some(Instant::now());
        self.debouncer.reset();
        self.last_poll = None;
        self.status_msg = format!(
            "Watching {} friend(s) for an open spot…",
            self.watched_order.len()
        );
    }

    fn stop_watching(&mut self) {
        self.watching = false;
        self.watch_started = None;
        if !self.needs_rewatch {
            self.status_msg = "Watching stopped.".into();
        }
    }

    fn stop_watching_after_join(&mut self, name: &str) {
        self.watching = false;
        self.watch_started = None;
        self.needs_rewatch = true;
        self.pending = None;
        self.alert_armed = false;
        self.status_msg = format!(
            "Joined via {name}. Watching stopped — click Start watching to look again."
        );
    }

    fn do_join(&mut self) {
        let Some(p) = self.pending.take() else {
            return;
        };
        match open_join(&p.method, p.steam_id) {
            Ok(()) => {
                self.stop_watching_after_join(&p.name);
            }
            Err(e) if e == "open_slots" => {
                if let Ok(session) = &self.steam {
                    session.open_friend_overlay(p.steam_id);
                }
                self.stop_watching_after_join(&p.name);
            }
            Err(e) => {
                self.status_msg = e;
                self.pending = Some(p);
            }
        }
    }

    fn dismiss_pending(&mut self) {
        self.pending = None;
        self.alert_armed = false;
        if self.watching {
            self.status_msg = "Join dismissed — still watching.".into();
        } else {
            self.status_msg = "Join dismissed.".into();
        }
    }

    fn presence_badge(p: &FriendPresence) -> (&'static str, Color32) {
        match p {
            FriendPresence::Joinable { .. } => ("JOINABLE", GREEN),
            FriendPresence::InCs2Full => ("IN GAME", AMBER),
            FriendPresence::OtherGame { .. } => ("OTHER", MUTED),
            FriendPresence::OfflineOrUnknown => ("AWAY", MUTED),
        }
    }

    fn watched_elapsed_secs(&self) -> Option<u64> {
        self.watch_started.map(|t| t.elapsed().as_secs())
    }

    fn friend_detail_line(&self, friend: &FriendInfo) -> String {
        if !friend.detail.is_empty() {
            return friend.detail.clone();
        }
        if let Some(d) = self.detail_cache.get(&friend.steam_id) {
            if !d.is_empty() {
                return d.clone();
            }
        }
        match friend.presence {
            FriendPresence::Joinable { .. } => "Spot available".into(),
            FriendPresence::InCs2Full => "In CS2 — no open spot signal".into(),
            _ => "In CS2".into(),
        }
    }
}

fn ingest_friends(
    ctx: &egui::Context,
    list: &[FriendInfo],
    names: &mut HashMap<u64, String>,
    details: &mut HashMap<u64, String>,
    avatars: &mut HashMap<u64, TextureHandle>,
) {
    for f in list {
        names.insert(f.steam_id, f.name.clone());
        if !f.detail.is_empty() {
            details.insert(f.steam_id, f.detail.clone());
        }
        if let Some(rgba) = &f.avatar_rgba {
            if rgba.len() == 64 * 64 * 4 {
                let image = egui::ColorImage::from_rgba_unmultiplied([64, 64], rgba);
                let tex = ctx.load_texture(
                    format!("avatar-{}", f.steam_id),
                    image,
                    TextureOptions::LINEAR,
                );
                avatars.insert(f.steam_id, tex);
            }
        }
    }
}

fn apply_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.window_fill = BG;
    visuals.panel_fill = BG;
    visuals.override_text_color = Some(TEXT);
    visuals.widgets.noninteractive.bg_fill = PANEL;
    visuals.widgets.inactive.bg_fill = PANEL_ALT;
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(36, 42, 48);
    visuals.widgets.active.bg_fill = Color32::from_rgb(48, 56, 64);
    visuals.selection.bg_fill = Color32::from_rgb(70, 52, 20);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, MUTED);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.hyperlink_color = AMBER;
    visuals.extreme_bg_color = PANEL;
    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    style.spacing.button_padding = Vec2::new(12.0, 6.0);
    ctx.set_style(style);
}

impl eframe::App for FriendwatchApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Ok(session) = &self.steam {
            session.run_callbacks();
        }

        let list_due = self
            .last_list_refresh
            .map(|t| t.elapsed() >= IDLE_REFRESH)
            .unwrap_or(true);
        if self.steam.is_ok() && list_due && !self.watching {
            self.refresh_cs2_list(ctx);
        }

        if self.watching {
            let due = self
                .last_poll
                .map(|t| t.elapsed() >= POLL_INTERVAL)
                .unwrap_or(true);
            if due {
                self.poll_once(ctx);
            }
            ctx.request_repaint_after(Duration::from_millis(200));
        } else if self.pending.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        } else if self.steam.is_ok() {
            ctx.request_repaint_after(Duration::from_secs(1));
        }

        // Always-on-top join alert window
        let mut join_clicked = false;
        let mut dismiss_clicked = false;
        if let Some(pending) = self.pending.clone() {
            let alert_id = egui::ViewportId::from_hash_of("cs2_friendwatch_join_alert");
            let should_focus = self.alert_armed;

            ctx.show_viewport_immediate(
                alert_id,
                egui::ViewportBuilder::default()
                    .with_title("CS2 Friendwatch — Spot available!")
                    .with_inner_size([440.0, 210.0])
                    .with_always_on_top()
                    .with_active(true)
                    .with_taskbar(true),
                |ctx, _class| {
                    apply_theme(ctx);
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE.fill(BG).inner_margin(20.0))
                        .show(ctx, |ui| {
                            ui.label(
                                RichText::new("Spot available!")
                                    .color(GREEN)
                                    .size(22.0)
                                    .strong(),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(&pending.name)
                                    .color(TEXT)
                                    .size(18.0)
                                    .strong(),
                            );
                            if !pending.detail.is_empty() {
                                ui.label(RichText::new(&pending.detail).color(MUTED).size(14.0));
                            }
                            ui.add_space(16.0);
                            ui.horizontal(|ui| {
                                if ui
                                    .add(
                                        egui::Button::new(RichText::new("Join now").size(16.0).strong())
                                            .fill(GREEN)
                                            .min_size(Vec2::new(140.0, 36.0)),
                                    )
                                    .clicked()
                                {
                                    join_clicked = true;
                                }
                                if ui
                                    .add(
                                        egui::Button::new(RichText::new("Dismiss").size(16.0))
                                            .min_size(Vec2::new(100.0, 36.0)),
                                    )
                                    .clicked()
                                {
                                    dismiss_clicked = true;
                                }
                            });
                        });
                    if should_focus {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                            egui::UserAttentionType::Critical,
                        ));
                        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                            egui::WindowLevel::AlwaysOnTop,
                        ));
                    }
                    ctx.request_repaint();
                },
            );

            if should_focus {
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                    egui::UserAttentionType::Informational,
                ));
                self.alert_armed = false;
            }
        }
        if join_clicked {
            self.do_join();
        } else if dismiss_clicked {
            self.dismiss_pending();
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(BG).inner_margin(16.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("CS2 Friendwatch")
                            .font(FontId::proportional(22.0))
                            .color(AMBER)
                            .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (label, color) = match self.steam_app_id {
                            Some(730) => ("APP 730", GREEN),
                            Some(480) => ("APP 480", AMBER),
                            Some(_) => ("STEAM", GREEN),
                            None => ("NO STEAM", RED),
                        };
                        ui.label(RichText::new(label).small().color(color).strong());
                    });
                });
                ui.add_space(2.0);
                ui.label(RichText::new(&self.status_msg).color(MUTED).size(13.0));
                ui.add_space(10.0);

                if self.steam.is_err() {
                    panel(ui, RED.linear_multiply(0.25), |ui| {
                        ui.label(
                            RichText::new("Steam is required. Start Steam, then restart this app.")
                                .color(RED),
                        );
                    });
                    return;
                }

                if self.pending.is_some() {
                    panel(ui, Color32::from_rgb(24, 48, 32), |ui| {
                        ui.label(
                            RichText::new("Join alert is open — use the popup window on top.")
                                .color(GREEN)
                                .size(14.0)
                                .strong(),
                        );
                    });
                    ui.add_space(10.0);
                }

                panel(ui, PANEL, |ui| {
                    ui.horizontal(|ui| {
                        if self.watching {
                            if ui
                                .add(egui::Button::new("Stop").fill(Color32::from_rgb(60, 36, 36)))
                                .clicked()
                            {
                                self.stop_watching();
                            }
                        } else if ui
                            .add_enabled(
                                !self.watched_order.is_empty(),
                                egui::Button::new(RichText::new("Start watching").strong()).fill(AMBER),
                            )
                            .clicked()
                        {
                            self.start_watching();
                        }

                        ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "{} selected · click a row to toggle",
                                    self.watched_order.len()
                                ))
                                .color(MUTED)
                                .size(12.0),
                            );
                            if let Some(t) = self.last_poll {
                                ui.label(
                                    RichText::new(format!(
                                        "Last poll {:.1}s ago",
                                        t.elapsed().as_secs_f32()
                                    ))
                                    .color(MUTED)
                                    .size(11.0),
                                );
                            } else if !self.watching {
                                ui.label(
                                    RichText::new("Idle refresh every 15s")
                                        .color(MUTED)
                                        .size(11.0),
                                );
                            }
                        });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if let Some(secs) = self.watched_elapsed_secs() {
                                ui.vertical(|ui| {
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::TOP),
                                        |ui| {
                                            ui.label(
                                                RichText::new("WATCHING")
                                                    .color(AMBER)
                                                    .small()
                                                    .strong(),
                                            );
                                        },
                                    );
                                    ui.label(
                                        RichText::new(format_elapsed(secs))
                                            .font(FontId::monospace(28.0))
                                            .color(TEXT)
                                            .strong(),
                                    );
                                });
                            } else if self.needs_rewatch {
                                ui.label(
                                    RichText::new("Start watching to continue")
                                        .color(AMBER)
                                        .size(13.0),
                                );
                            }
                        });
                    });
                });

                ui.add_space(12.0);
                ui.label(
                    RichText::new("Friends in Counter-Strike 2")
                        .color(TEXT)
                        .size(14.0)
                        .strong(),
                );
                ui.label(
                    RichText::new("Click anywhere on a row to watch / unwatch.")
                        .color(MUTED)
                        .size(12.0),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Filter").color(MUTED).size(12.0));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.filter)
                            .desired_width(220.0)
                            .hint_text("name…"),
                    );
                    ui.label(
                        RichText::new(format!("{} online", self.cs2_friends.len()))
                            .color(MUTED)
                            .size(12.0),
                    );
                });
                ui.add_space(6.0);

                let filter = self.filter.to_lowercase();
                let rows: Vec<FriendInfo> = self
                    .cs2_friends
                    .iter()
                    .filter(|f| filter.is_empty() || f.name.to_lowercase().contains(&filter))
                    .cloned()
                    .collect();

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .max_height((ui.available_height() - 100.0).max(120.0))
                    .show(ui, |ui| {
                        if rows.is_empty() {
                            panel(ui, PANEL, |ui| {
                                ui.label(
                                    RichText::new("No friends in CS2 match this filter.")
                                        .color(MUTED),
                                );
                            });
                        }

                        let mut toggles: Vec<(u64, String, bool)> = Vec::new();
                        for friend in &rows {
                            let checked = self.watched_set.contains(&friend.steam_id);
                            let (badge, badge_color) = Self::presence_badge(&friend.presence);
                            let detail = self.friend_detail_line(friend);
                            let avatar = self.avatar_tex.get(&friend.steam_id).cloned();

                            let width = ui.available_width();
                            let (rect, response) =
                                ui.allocate_exact_size(Vec2::new(width, ROW_HEIGHT), Sense::click());
                            let hovered = response.hovered();
                            if response.clicked() {
                                toggles.push((friend.steam_id, friend.name.clone(), !checked));
                            }

                            let fill = if checked {
                                Color32::from_rgb(32, 38, 28)
                            } else if hovered {
                                PANEL_ALT
                            } else {
                                PANEL
                            };
                            ui.painter().rect(
                                rect,
                                4.0,
                                fill,
                                Stroke::new(1.0, BORDER),
                                StrokeKind::Inside,
                            );

                            let mut x = rect.left() + 10.0;
                            let cy = rect.center().y;

                            let box_size = 16.0;
                            let box_rect = Rect::from_center_size(
                                Pos2::new(x + box_size / 2.0, cy),
                                Vec2::splat(box_size),
                            );
                            ui.painter().rect(
                                box_rect,
                                2.0,
                                if checked {
                                    AMBER
                                } else {
                                    Color32::TRANSPARENT
                                },
                                Stroke::new(1.5, if checked { AMBER } else { MUTED }),
                                StrokeKind::Inside,
                            );
                            if checked {
                                ui.painter().line_segment(
                                    [
                                        Pos2::new(box_rect.left() + 3.0, cy),
                                        Pos2::new(box_rect.center().x - 1.0, box_rect.bottom() - 4.0),
                                    ],
                                    Stroke::new(2.0, BG),
                                );
                                ui.painter().line_segment(
                                    [
                                        Pos2::new(box_rect.center().x - 1.0, box_rect.bottom() - 4.0),
                                        Pos2::new(box_rect.right() - 3.0, box_rect.top() + 3.0),
                                    ],
                                    Stroke::new(2.0, BG),
                                );
                            }
                            x += box_size + 10.0;

                            let av_rect = Rect::from_center_size(
                                Pos2::new(x + AVATAR_SIZE / 2.0, cy),
                                Vec2::splat(AVATAR_SIZE),
                            );
                            if let Some(tex) = avatar {
                                ui.painter().image(
                                    tex.id(),
                                    av_rect,
                                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                    Color32::WHITE,
                                );
                            } else {
                                ui.painter().rect_filled(av_rect, 4.0, PANEL_ALT);
                                ui.painter().text(
                                    av_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "?",
                                    FontId::proportional(16.0),
                                    MUTED,
                                );
                            }
                            x += AVATAR_SIZE + 10.0;

                            let text_top = rect.top() + 10.0;
                            ui.painter().text(
                                Pos2::new(x, text_top),
                                egui::Align2::LEFT_TOP,
                                &friend.name,
                                FontId::proportional(14.0),
                                TEXT,
                            );
                            let name_width = friend.name.len() as f32 * 7.5;
                            ui.painter().text(
                                Pos2::new(x + name_width + 10.0, text_top + 2.0),
                                egui::Align2::LEFT_TOP,
                                badge,
                                FontId::proportional(11.0),
                                badge_color,
                            );
                            ui.painter().text(
                                Pos2::new(x, text_top + 20.0),
                                egui::Align2::LEFT_TOP,
                                detail,
                                FontId::proportional(12.0),
                                MUTED,
                            );
                        }
                        for (id, name, checked) in toggles {
                            self.toggle_watch(id, &name, checked);
                        }
                    });

                if !self.watched_order.is_empty() {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Watch queue")
                            .color(TEXT)
                            .size(13.0)
                            .strong(),
                    );
                    let status_by_id: HashMap<u64, &WatchedFriendStatus> =
                        self.statuses.iter().map(|s| (s.steam_id, s)).collect();
                    for (i, id) in self.watched_order.iter().enumerate() {
                        let name = self
                            .name_cache
                            .get(id)
                            .cloned()
                            .unwrap_or_else(|| id.to_string());
                        let (badge, detail, color) = if let Some(s) = status_by_id.get(id) {
                            let (b, c) = Self::presence_badge(&s.presence);
                            let d = if s.detail.is_empty() {
                                self.detail_cache
                                    .get(id)
                                    .cloned()
                                    .filter(|d| !d.is_empty())
                                    .unwrap_or_else(|| presence_fallback(&s.presence).to_string())
                            } else {
                                s.detail.clone()
                            };
                            (b, d, c)
                        } else if let Some(f) = self.cs2_friends.iter().find(|f| f.steam_id == *id) {
                            let (b, c) = Self::presence_badge(&f.presence);
                            (b, self.friend_detail_line(f), c)
                        } else {
                            ("WAITING", "Not in CS2".into(), MUTED)
                        };
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{}.", i + 1))
                                    .color(MUTED)
                                    .monospace()
                                    .size(12.0),
                            );
                            ui.label(RichText::new(name).color(TEXT).size(12.0));
                            ui.label(RichText::new(badge).color(color).small().strong());
                            ui.label(RichText::new(detail).color(MUTED).size(11.0));
                        });
                    }
                }
            });
    }
}

fn presence_fallback(p: &FriendPresence) -> &'static str {
    match p {
        FriendPresence::Joinable { .. } => "Spot available",
        FriendPresence::InCs2Full => "In CS2 — no open spot",
        FriendPresence::OtherGame { .. } => "Other game",
        FriendPresence::OfflineOrUnknown => "Offline / unknown",
    }
}

fn panel(ui: &mut egui::Ui, fill: Color32, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, BORDER))
        .inner_margin(egui::Margin::symmetric(12, 10))
        .corner_radius(4.0)
        .show(ui, add_contents);
}

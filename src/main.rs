#![windows_subsystem = "windows"]

mod auth;
mod launcher;
mod store;
mod tray;

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CornerRadius, FontId, Margin, RichText, Stroke, StrokeKind,
};
use std::path::Path;
use std::time::{Duration, Instant};
use store::Entry;

// ────────────────────────────── 设计令牌（与 D:\code\ui.md 对齐） ──────────────────────────────
const BG_APP: Color32 = Color32::from_rgb(0x0F, 0x11, 0x17);       // --bg-base
const BG_SURFACE: Color32 = Color32::from_rgb(0x15, 0x18, 0x22);   // --bg-panel
const BG_SURFACE_2: Color32 = Color32::from_rgb(0x1C, 0x20, 0x30); // --bg-elev
const BG_ELEVATED: Color32 = Color32::from_rgb(0x1C, 0x20, 0x30);  // --bg-elev
const BG_HOVER: Color32 = Color32::from_rgb(0x23, 0x28, 0x38);     // --bg-hover
const BG_ACTIVE: Color32 = Color32::from_rgb(0x2A, 0x30, 0x46);    // --bg-active
const BORDER: Color32 = Color32::from_rgb(0x27, 0x2C, 0x3C);       // --border
const BORDER_STRONG: Color32 = Color32::from_rgb(0x36, 0x3C, 0x50); // --border-strong
const BORDER_HOVER: Color32 = Color32::from_rgb(0x4A, 0x52, 0x70);
const TEXT: Color32 = Color32::from_rgb(0xE7, 0xEA, 0xF2);         // --text
const TEXT_MUTED: Color32 = Color32::from_rgb(0x99, 0xA1, 0xB7);   // --text-dim
const TEXT_FAINT: Color32 = Color32::from_rgb(0x6B, 0x73, 0x89);   // --text-faint
const ACCENT: Color32 = Color32::from_rgb(0x5B, 0x8C, 0xFF);       // --accent
const ACCENT_HOVER: Color32 = Color32::from_rgb(0x7A, 0xA0, 0xFF); // --accent-hover
const ACCENT_ACTIVE: Color32 = Color32::from_rgb(0x4A, 0x78, 0xE8);
const ACCENT_SOFT: Color32 = Color32::from_rgba_unmultiplied_const(0x5B, 0x8C, 0xFF, 36);
const ACCENT_BORDER: Color32 = Color32::from_rgba_unmultiplied_const(0x5B, 0x8C, 0xFF, 90);
const ACCENT_DASH: Color32 = Color32::from_rgba_unmultiplied_const(0x5B, 0x8C, 0xFF, 150);
const SUCCESS: Color32 = Color32::from_rgb(0x3F, 0xB9, 0x50);      // --success
const DANGER: Color32 = Color32::from_rgb(0xF8, 0x51, 0x49);       // --danger
const DANGER_HOVER: Color32 = Color32::from_rgb(0xFF, 0x6B, 0x63);
const DANGER_ACTIVE: Color32 = Color32::from_rgb(0xDA, 0x3B, 0x34);
const DANGER_SOFT: Color32 = Color32::from_rgba_unmultiplied_const(0xF8, 0x51, 0x49, 31);
const WARN: Color32 = Color32::from_rgb(0xD2, 0x99, 0x22);         // --warn

const ROW_H: f32 = 48.0;
const HEADER_H: f32 = 42.0;
const AUTO_LOCK_SECS: u64 = 10 * 60;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("RDP 管理器")
            .with_inner_size([1080.0, 680.0])
            .with_min_inner_size([900.0, 520.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "RDP 管理器",
        options,
        Box::new(|cc| {
            setup_fonts(&cc.egui_ctx);
            configure_style(&cc.egui_ctx);
            Ok(Box::new(App::new(cc.egui_ctx.clone())) as Box<dyn eframe::App>)
        }),
    )
}

/// egui 内置字体只包含拉丁/西里尔字形，中文会渲染成方框。
/// 这里从系统字体里加载一个中文字体作为后备字体，挂到默认字体之后。
fn setup_fonts(ctx: &egui::Context) {
    let Some(bytes) = load_cjk_font() else {
        eprintln!("找不到中文字体，界面中文会显示为方框。请安装微软雅黑 / 黑体 / Noto Sans CJK 等字体。");
        return;
    };

    use egui::epaint::text::{FontInsert, FontPriority, InsertFontFamily};
    let families = vec![
        InsertFontFamily {
            family: egui::FontFamily::Proportional,
            priority: FontPriority::Lowest,
        },
        InsertFontFamily {
            family: egui::FontFamily::Monospace,
            priority: FontPriority::Lowest,
        },
    ];
    ctx.add_font(FontInsert::new(
        "cjk_fallback",
        egui::FontData::from_owned(bytes),
        families,
    ));
}

fn load_cjk_font() -> Option<Vec<u8>> {
    let candidates: Vec<std::path::PathBuf> = if cfg!(windows) {
        let fonts = std::path::PathBuf::from(
            std::env::var_os("WINDIR").unwrap_or_else(|| "C:/Windows".into()),
        )
        .join("Fonts");
        ["msyh.ttc", "simhei.ttf", "simsun.ttc"]
            .iter()
            .map(|f| fonts.join(f))
            .collect()
    } else if cfg!(target_os = "macos") {
        [
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/STHeiti Light.ttc",
        ]
        .iter()
        .map(std::path::PathBuf::from)
        .collect()
    } else {
        [
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
        ]
        .iter()
        .map(std::path::PathBuf::from)
        .collect()
    };

    candidates.iter().find_map(|p| std::fs::read(p).ok())
}

/// 一套深色现代主题的全局样式：柔和的表面层级、统一的圆角与间距。
fn configure_style(ctx: &egui::Context) {
    use egui::style::WidgetVisuals;
    use egui::TextStyle;

    ctx.all_styles_mut(|style| {
    let mut v = egui::Visuals::dark();
    v.dark_mode = true;
    v.panel_fill = BG_APP;
    v.window_fill = BG_SURFACE;
    v.extreme_bg_color = BG_APP;
    v.faint_bg_color = BG_SURFACE_2;
    v.code_bg_color = BG_SURFACE_2;
    v.text_edit_bg_color = Some(BG_APP);
    v.override_text_color = Some(TEXT);
    v.weak_text_color = Some(TEXT_MUTED);
    v.hyperlink_color = ACCENT;
    v.warn_fg_color = WARN;
    v.error_fg_color = DANGER;
    v.selection.bg_fill = ACCENT_SOFT;
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.window_corner_radius = CornerRadius::same(14);
    v.menu_corner_radius = CornerRadius::same(10);
    v.window_stroke = Stroke::new(1.0, BORDER_STRONG);
    v.window_shadow = egui::Shadow {
        offset: [0, 8],
        blur: 28,
        spread: 0,
        color: Color32::from_rgba_unmultiplied_const(0, 0, 0, 97),
    };
    v.popup_shadow = v.window_shadow;
    v.button_frame = true;
    v.striped = true;

    let border = Stroke::new(1.0, BORDER_STRONG);
    let radius = CornerRadius::same(6);
    let base = WidgetVisuals {
        bg_fill: BG_ELEVATED,
        weak_bg_fill: BG_ELEVATED,
        bg_stroke: border,
        corner_radius: radius,
        fg_stroke: Stroke::new(1.0, TEXT),
        expansion: 0.0,
    };
    v.widgets.inactive = base;
    v.widgets.hovered = WidgetVisuals {
        weak_bg_fill: BG_HOVER,
        bg_stroke: Stroke::new(1.0, BORDER_STRONG),
        ..base
    };
    v.widgets.active = WidgetVisuals {
        weak_bg_fill: ACCENT_SOFT,
        bg_stroke: Stroke::new(1.0, ACCENT),
        ..base
    };
    v.widgets.open = v.widgets.active;
    v.widgets.noninteractive = WidgetVisuals {
        bg_fill: BG_SURFACE,
        weak_bg_fill: Color32::TRANSPARENT,
        bg_stroke: border,
        corner_radius: radius,
        fg_stroke: Stroke::new(1.0, TEXT),
        expansion: 0.0,
    };
    style.visuals = v;

    style.spacing.item_spacing = vec2(10.0, 8.0);
    style.spacing.button_padding = vec2(12.0, 7.0);
    style.spacing.window_margin = Margin::same(24);
    style.spacing.menu_margin = Margin::same(16);
    style.spacing.interact_size = vec2(40.0, 32.0);

    for (key, size) in [
        (TextStyle::Heading, 19.0),
        (TextStyle::Body, 13.5),
        (TextStyle::Button, 13.5),
        (TextStyle::Small, 11.5),
    ] {
        if let Some(font) = style.text_styles.get_mut(&key) {
            font.size = size;
        }
    }
    });
}

// ────────────────────────────── 图标绘制 ──────────────────────────────
#[derive(Clone, Copy)]
enum Icon {
    Monitor,
    Plus,
    Import,
    Upload,
    Search,
    Play,
    Trash,
    Close,
    Edit,
    Lock,
}

fn paint_icon(p: &egui::Painter, icon: Icon, c: egui::Pos2, s: f32, color: Color32) {
    let st = Stroke::new(1.6, color);
    let h = s / 2.0;
    match icon {
        Icon::Monitor => {
            let r = egui::Rect::from_center_size(
                pos2(c.x, c.y - s * 0.08),
                vec2(s, s * 0.68),
            );
            p.rect_stroke(r, CornerRadius::same(2), st, StrokeKind::Inside);
            p.line_segment([pos2(c.x, r.max.y), pos2(c.x, r.max.y + s * 0.12)], st);
            p.line_segment(
                [
                    pos2(c.x - s * 0.20, r.max.y + s * 0.13),
                    pos2(c.x + s * 0.20, r.max.y + s * 0.13),
                ],
                st,
            );
        }
        Icon::Plus => {
            p.line_segment([pos2(c.x - h * 0.7, c.y), pos2(c.x + h * 0.7, c.y)], st);
            p.line_segment([pos2(c.x, c.y - h * 0.7), pos2(c.x, c.y + h * 0.7)], st);
        }
        Icon::Import => {
            p.line_segment([pos2(c.x, c.y - h * 0.75), pos2(c.x, c.y + h * 0.18)], st);
            p.line_segment(
                [pos2(c.x - h * 0.35, c.y - h * 0.12), pos2(c.x, c.y + h * 0.18)],
                st,
            );
            p.line_segment(
                [pos2(c.x + h * 0.35, c.y - h * 0.12), pos2(c.x, c.y + h * 0.18)],
                st,
            );
            paint_tray(p, c, s, st);
        }
        Icon::Upload => {
            p.line_segment([pos2(c.x, c.y + h * 0.18), pos2(c.x, c.y - h * 0.75)], st);
            p.line_segment(
                [pos2(c.x - h * 0.35, c.y - h * 0.45), pos2(c.x, c.y - h * 0.75)],
                st,
            );
            p.line_segment(
                [pos2(c.x + h * 0.35, c.y - h * 0.45), pos2(c.x, c.y - h * 0.75)],
                st,
            );
            paint_tray(p, c, s, st);
        }
        Icon::Search => {
            let cc = pos2(c.x - s * 0.08, c.y - s * 0.08);
            p.circle_stroke(cc, s * 0.30, st);
            p.line_segment(
                [
                    pos2(cc.x + s * 0.21, cc.y + s * 0.21),
                    pos2(c.x + s * 0.36, c.y + s * 0.36),
                ],
                st,
            );
        }
        Icon::Play => {
            let a = pos2(c.x - s * 0.22, c.y - s * 0.30);
            let b = pos2(c.x - s * 0.22, c.y + s * 0.30);
            let d = pos2(c.x + s * 0.30, c.y);
            p.line_segment([a, b], st);
            p.line_segment([a, d], st);
            p.line_segment([b, d], st);
        }
        Icon::Trash => {
            let r = egui::Rect::from_center_size(
                pos2(c.x, c.y + s * 0.10),
                vec2(s * 0.66, s * 0.60),
            );
            p.rect_stroke(r, CornerRadius::same(2), st, StrokeKind::Inside);
            p.line_segment(
                [pos2(c.x - s * 0.30, r.min.y), pos2(c.x + s * 0.30, r.min.y)],
                st,
            );
            p.line_segment(
                [
                    pos2(c.x - s * 0.14, r.min.y - s * 0.10),
                    pos2(c.x + s * 0.14, r.min.y - s * 0.10),
                ],
                st,
            );
            p.line_segment(
                [pos2(c.x, r.min.y + s * 0.16), pos2(c.x, r.max.y - s * 0.10)],
                st,
            );
        }
        Icon::Close => {
            p.line_segment(
                [pos2(c.x - h * 0.55, c.y - h * 0.55), pos2(c.x + h * 0.55, c.y + h * 0.55)],
                st,
            );
            p.line_segment(
                [pos2(c.x + h * 0.55, c.y - h * 0.55), pos2(c.x - h * 0.55, c.y + h * 0.55)],
                st,
            );
        }
        Icon::Edit => {
            // Pencil icon
            let tip = pos2(c.x - h * 0.55, c.y + h * 0.55);
            let top = pos2(c.x + h * 0.35, c.y - h * 0.75);
            let off = vec2(h * 0.28, h * 0.28);
            p.line_segment([tip, top], st);
            p.line_segment([top, pos2(top.x + off.x, top.y + off.y)], st);
            p.line_segment([pos2(top.x + off.x, top.y + off.y), pos2(tip.x + off.x, tip.y + off.y)], st);
            p.line_segment([pos2(tip.x + off.x, tip.y + off.y), tip], st);
        }
        Icon::Lock => {
            let body = egui::Rect::from_center_size(
                pos2(c.x, c.y + s * 0.16),
                vec2(s * 0.66, s * 0.50),
            );
            p.rect_stroke(body, CornerRadius::same(2), st, StrokeKind::Inside);
            let shackle = egui::Rect::from_center_size(
                pos2(c.x, body.min.y - s * 0.14),
                vec2(s * 0.38, s * 0.30),
            );
            p.rect_stroke(shackle, CornerRadius::same(4), st, StrokeKind::Inside);
        }
    }
}

fn paint_tray(p: &egui::Painter, c: egui::Pos2, s: f32, st: Stroke) {
    let h = s / 2.0;
    let y = c.y + h * 0.62;
    p.line_segment([pos2(c.x - h * 0.78, y), pos2(c.x + h * 0.78, y)], st);
    p.line_segment(
        [pos2(c.x - h * 0.78, y), pos2(c.x - h * 0.78, y - h * 0.34)],
        st,
    );
    p.line_segment(
        [pos2(c.x + h * 0.78, y), pos2(c.x + h * 0.78, y - h * 0.34)],
        st,
    );
}

// ────────────────────────────── 按钮控件 ──────────────────────────────
#[derive(Clone, Copy, PartialEq, Eq)]
enum BtnKind {
    Primary,
    Secondary,
    Ghost,
    DangerSolid,
}

impl BtnKind {
    fn text_color(self) -> Color32 {
        match self {
            BtnKind::Primary | BtnKind::DangerSolid => Color32::WHITE,
            BtnKind::Secondary => TEXT,
            BtnKind::Ghost => TEXT_MUTED,
        }
    }

    fn bg(self, hover: bool, down: bool) -> Color32 {
        match (self, hover, down) {
            (BtnKind::Primary, _, true) => ACCENT_ACTIVE,
            (BtnKind::Primary, true, _) => ACCENT_HOVER,
            (BtnKind::Primary, _, _) => ACCENT,
            (BtnKind::Secondary, true, _) => BG_HOVER,
            (BtnKind::Secondary, _, _) => BG_ELEVATED,
            (BtnKind::Ghost, true, _) => BG_HOVER,
            (BtnKind::Ghost, _, _) => Color32::TRANSPARENT,
            (BtnKind::DangerSolid, _, true) => DANGER_ACTIVE,
            (BtnKind::DangerSolid, true, _) => DANGER_HOVER,
            (BtnKind::DangerSolid, _, _) => DANGER,
        }
    }

    fn stroke(self, hover: bool) -> Option<Stroke> {
        match self {
            BtnKind::Secondary => Some(Stroke::new(
                1.0,
                if hover { BORDER_HOVER } else { BORDER_STRONG },
            )),
            BtnKind::Primary => Some(Stroke::new(
                1.0,
                if hover { ACCENT_HOVER } else { ACCENT },
            )),
            _ => None,
        }
    }
}

struct ModernButton<'a> {
    label: &'a str,
    icon: Option<Icon>,
    kind: BtnKind,
    min_w: f32,
    height: f32,
}

impl<'a> ModernButton<'a> {
    fn new(label: &'a str) -> Self {
        Self {
            label,
            icon: None,
            kind: BtnKind::Secondary,
            min_w: 0.0,
            height: 32.0,
        }
    }

    fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    fn kind(mut self, kind: BtnKind) -> Self {
        self.kind = kind;
        self
    }

    fn min_w(mut self, w: f32) -> Self {
        self.min_w = w;
        self
    }

    fn height(mut self, h: f32) -> Self {
        self.height = h;
        self
    }
}

impl egui::Widget for ModernButton<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let font = FontId::proportional(13.5);
        let color = self.kind.text_color();
        let text_w = if self.label.is_empty() {
            0.0
        } else {
            ui.painter()
                .layout_no_wrap(self.label.to_owned(), font.clone(), color)
                .size()
                .x
        };
        let icon_gap = if self.icon.is_some() { 20.0 } else { 0.0 };
        let w = (text_w + icon_gap + 26.0).max(self.min_w);

        let (rect, response) =
            ui.allocate_exact_size(vec2(w, self.height), egui::Sense::click());
        if ui.is_rect_visible(rect) {
            let hover = response.hovered();
            let down = response.is_pointer_button_down_on();
            ui.painter()
                .rect_filled(rect, CornerRadius::same(6), self.kind.bg(hover, down));
            if let Some(stroke) = self.kind.stroke(hover) {
                ui.painter()
                    .rect_stroke(rect, CornerRadius::same(6), stroke, StrokeKind::Inside);
            }
            if let Some(ic) = self.icon {
                let mut x = rect.min.x + 13.0;
                paint_icon(
                    ui.painter(),
                    ic,
                    pos2(x + 6.0, rect.center().y),
                    14.0,
                    color,
                );
                x += icon_gap;
                if !self.label.is_empty() {
                    ui.painter().text(
                        pos2(x, rect.center().y),
                        Align2::LEFT_CENTER,
                        self.label,
                        font,
                        color,
                    );
                }
            } else if !self.label.is_empty() {
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    self.label,
                    font,
                    color,
                );
            }
        }
        response
    }
}

struct IconButton {
    icon: Icon,
    danger: bool,
    tip: &'static str,
}

impl egui::Widget for IconButton {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(vec2(30.0, 30.0), egui::Sense::click());
        if ui.is_rect_visible(rect) {
            let hover = response.hovered();
            let bg = match (hover, self.danger) {
                (true, true) => DANGER_SOFT,
                (true, false) => BG_HOVER,
                _ => Color32::TRANSPARENT,
            };
            ui.painter()
                .rect_filled(rect, CornerRadius::same(7), bg);
            let color = match (hover, self.danger) {
                (true, true) => DANGER,
                (true, false) => TEXT,
                _ => TEXT_MUTED,
            };
            paint_icon(ui.painter(), self.icon, rect.center(), 15.0, color);
        }
        response.on_hover_text(self.tip)
    }
}

// ────────────────────────────── 小工具 ──────────────────────────────
fn avatar_color(s: &str) -> Color32 {
    const COLORS: [Color32; 6] = [
        Color32::from_rgb(0x5B, 0x7C, 0xFA),
        Color32::from_rgb(0x2E, 0xB8, 0x9E),
        Color32::from_rgb(0x9B, 0x6D, 0xF0),
        Color32::from_rgb(0xE0, 0x9B, 0x4A),
        Color32::from_rgb(0xE5, 0x6B, 0x8C),
        Color32::from_rgb(0x3E, 0xA8, 0xD8),
    ];
    let hash = s
        .bytes()
        .fold(0usize, |a, b| a.wrapping_mul(31).wrapping_add(b as usize));
    COLORS[hash % COLORS.len()]
}

fn modal_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(BG_SURFACE)
        .stroke(Stroke::new(1.0, BORDER_STRONG))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(Margin::same(20))
        .shadow(egui::Shadow {
            offset: [0, 8],
            blur: 28,
            spread: 0,
            color: Color32::from_rgba_unmultiplied_const(0, 0, 0, 97),
        })
}

fn form_field(
    ui: &mut egui::Ui,
    label: &'static str,
    hint: &'static str,
    text: &mut String,
    id: egui::Id,
) {
    form_field_inner(ui, label, hint, text, id, false);
}

fn form_field_password(
    ui: &mut egui::Ui,
    label: &'static str,
    hint: &'static str,
    text: &mut String,
    id: egui::Id,
) {
    form_field_inner(ui, label, hint, text, id, true);
}

fn form_field_inner(
    ui: &mut egui::Ui,
    label: &'static str,
    hint: &'static str,
    text: &mut String,
    id: egui::Id,
    password: bool,
) {
    ui.label(RichText::new(label).size(12.0).strong().color(TEXT_MUTED));
    ui.add_space(4.0);
    let focused = ui.ctx().memory(|m| m.focused() == Some(id));
    let frame = egui::Frame::new()
        .fill(BG_APP)
        .stroke(Stroke::new(1.0, if focused { ACCENT } else { BORDER_STRONG }))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(11, 7));
    let width = ui.available_width();
    ui.add_sized(
        [width, 32.0],
        egui::TextEdit::singleline(text)
            .id(id)
            .frame(frame)
            .hint_text(hint)
            .text_color(TEXT)
            .password(password),
    );
}

/// 表格的列宽与起始位置（相对行左边缘的偏移）。
struct Columns {
    pad: f32,
    icon: f32,
    name: f32,
    host: f32,
    user: f32,
    actions: f32,
    x_name: f32,
    x_host: f32,
    x_user: f32,
}

impl Columns {
    fn new(full_w: f32) -> Self {
        let pad = 18.0;
        let icon = 46.0;
        let actions = 212.0;
        let inner = (full_w - pad * 2.0 - icon - actions).max(240.0);
        let name = inner * 0.30;
        let host = inner * 0.42;
        let user = inner * 0.28;
        let x_name = pad + icon;
        let x_host = x_name + name;
        let x_user = x_host + host;
        Self {
            pad,
            icon,
            name,
            host,
            user,
            actions,
            x_name,
            x_host,
            x_user,
        }
    }
}

enum RowAction {
    Connect,
    Edit,
    Delete,
}

struct App {
    store: store::Store,
    filter: String,
    selected: Option<String>,

    show_new: bool,
    form_name: String,
    form_host: String,
    form_user: String,
    form_pass: String,
    form_error: Option<String>,
    form_focus_pending: bool,

    editing_id: Option<String>,
    edit_name: String,
    edit_host: String,
    edit_user: String,
    edit_pass: String,
    edit_error: Option<String>,
    edit_focus_pending: bool,

    confirm_delete: Option<String>,
    status: Option<(String, bool, Instant)>,

    auth: auth::MasterAuth,
    unlocked: bool,
    last_activity: Instant,
    lock_pass: String,
    lock_confirm: String,
    lock_error: Option<String>,
    lock_focus_pending: bool,

    show_master_modal: bool,
    master_old: String,
    master_new: String,
    master_confirm: String,
    master_error: Option<String>,
    master_focus_pending: bool,

    tray: tray::TrayHandle,
    confirm_exit: bool,
    hidden_to_tray: bool,
}

impl App {
    fn new(egui_ctx: egui::Context) -> Self {
        let mut auth_err = None;
        let auth = match auth::MasterAuth::load() {
            Ok(a) => a,
            Err(e) => {
                auth_err = Some(e);
                auth::MasterAuth::empty()
            }
        };
        let unlocked = !auth.is_enabled() && !auth.needs_initial_setup();

        let mut app = Self {
            store: store::Store::empty(),
            filter: String::new(),
            selected: None,
            show_new: false,
            form_name: String::new(),
            form_host: String::new(),
            form_user: String::new(),
            form_pass: String::new(),
            form_error: None,
            form_focus_pending: false,
            editing_id: None,
            edit_name: String::new(),
            edit_host: String::new(),
            edit_user: String::new(),
            edit_pass: String::new(),
            edit_error: None,
            edit_focus_pending: false,
            confirm_delete: None,
            status: None,
            auth,
            unlocked,
            last_activity: Instant::now(),
            lock_pass: String::new(),
            lock_confirm: String::new(),
            lock_error: auth_err,
            lock_focus_pending: !unlocked,
            show_master_modal: false,
            master_old: String::new(),
            master_new: String::new(),
            master_confirm: String::new(),
            master_error: None,
            master_focus_pending: false,
            tray: tray::TrayHandle::start(egui_ctx),
            confirm_exit: false,
            hidden_to_tray: false,
        };
        match store::Store::load() {
            Ok(s) => app.store = s,
            Err(e) => app.notify(e, true),
        }
        app
    }

    fn lock_now(&mut self) {
        self.unlocked = false;
        self.lock_pass.clear();
        self.lock_confirm.clear();
        self.lock_error = None;
        self.lock_focus_pending = true;
        self.show_new = false;
        self.form_pass.clear();
        self.editing_id = None;
        self.edit_pass.clear();
        self.confirm_delete = None;
        self.show_master_modal = false;
        self.master_old.clear();
        self.master_new.clear();
        self.master_confirm.clear();
        self.master_error = None;
    }

    fn open_master_modal(&mut self) {
        self.show_master_modal = true;
        self.master_old.clear();
        self.master_new.clear();
        self.master_confirm.clear();
        self.master_error = None;
        self.master_focus_pending = true;
    }

    fn notify(&mut self, text: impl Into<String>, is_error: bool) {
        self.status = Some((text.into(), is_error, Instant::now()));
    }

    fn import_path(&mut self, path: &Path) {
        match self.store.import_path(path) {
            Ok(entry) => {
                self.selected = Some(entry.id.clone());
                self.notify(format!("已导入 {}", entry.name), false);
            }
            Err(e) => self.notify(e, true),
        }
    }

    fn import_bytes(&mut self, name: &str, bytes: &[u8]) {
        match self.store.import_bytes(name, bytes) {
            Ok(entry) => {
                self.selected = Some(entry.id.clone());
                self.notify(format!("已导入 {}", entry.name), false);
            }
            Err(e) => self.notify(e, true),
        }
    }

    fn handle_dropped(&mut self, files: Vec<egui::DroppedFile>) {
        for file in files {
            if let Some(path) = file.path {
                self.import_path(&path);
            } else if let Some(bytes) = file.bytes {
                let name = if file.name.is_empty() {
                    "dropped.rdp".to_string()
                } else {
                    file.name.clone()
                };
                self.import_bytes(&name, &bytes);
            }
        }
    }

    fn pick_files(&mut self) {
        let picked = rfd::FileDialog::new()
            .set_title("选择 .rdp 文件")
            .add_filter("远程桌面连接", &["rdp"])
            .pick_files();
        if let Some(paths) = picked {
            for path in paths {
                self.import_path(&path);
            }
        }
    }

    fn launch(&mut self, entry: &Entry) {
        let path = self.store.rdp_path(entry);
        match launcher::launch(&path, &entry.host, &entry.username, &entry.password) {
            Ok(()) => self.notify(format!("已启动 {}", entry.name), false),
            Err(e) => self.notify(e, true),
        }
    }

    fn save_new(&mut self) {
        match self.store.create(
            self.form_name.clone(),
            self.form_host.clone(),
            self.form_user.clone(),
            self.form_pass.clone(),
        ) {
            Ok(entry) => {
                self.selected = Some(entry.id.clone());
                self.form_name.clear();
                self.form_host.clear();
                self.form_user.clear();
                self.form_pass.clear();
                self.form_error = None;
                self.form_focus_pending = false;
                self.show_new = false;
                self.notify(format!("已新增 {}", entry.name), false);
            }
            Err(e) => self.form_error = Some(e),
        }
    }

    fn open_edit_form(&mut self, entry: &Entry) {
        self.editing_id = Some(entry.id.clone());
        self.edit_name = entry.name.clone();
        self.edit_host = entry.host.clone();
        self.edit_user = entry.username.clone();
        self.edit_pass = entry.password.clone();
        self.edit_error = None;
        self.edit_focus_pending = true;
    }

    fn save_edit(&mut self) {
        let Some(id) = self.editing_id.clone() else {
            return;
        };
        match self.store.update(
            &id,
            self.edit_name.clone(),
            self.edit_host.clone(),
            self.edit_user.clone(),
            self.edit_pass.clone(),
        ) {
            Ok(entry) => {
                self.selected = Some(entry.id.clone());
                self.editing_id = None;
                self.edit_error = None;
                self.edit_focus_pending = false;
                self.notify(format!("已更新 {}", entry.name), false);
            }
            Err(e) => self.edit_error = Some(e),
        }
    }

    fn delete(&mut self, id: &str) {
        match self.store.remove(id) {
            Ok(()) => {
                if self.selected.as_deref() == Some(id) {
                    self.selected = None;
                }
                self.notify("已删除", false);
            }
            Err(e) => self.notify(e, true),
        }
    }

    fn open_new_form(&mut self) {
        self.show_new = true;
        self.form_error = None;
        self.form_focus_pending = true;
    }

    // ── 顶栏 ────────────────────────────────────────────────────────
    fn draw_header(&mut self, ui: &mut egui::Ui) {
        let count = self.store.entries.len();
        let mut filter = std::mem::take(&mut self.filter);
        let mut clicked_new = false;
        let mut clicked_import = false;
        let mut clicked_master = false;
        let mut clicked_lock = false;
        let master_enabled = self.auth.is_enabled();

        egui::Panel::top(egui::Id::new("app_header"))
            .frame(
                egui::Frame::new()
                    .fill(BG_SURFACE)
                    .inner_margin(Margin::symmetric(14, 8)),
            )
            .show(ui, |ui| {
                let panel_rect = ui.max_rect();
                ui.painter().hline(
                    panel_rect.x_range(),
                    panel_rect.max.y + 8.0,
                    Stroke::new(1.0, BORDER),
                );
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 9.0;

                    let (logo, _) =
                        ui.allocate_exact_size(vec2(22.0, 22.0), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(logo, CornerRadius::same(7), ACCENT);
                    paint_icon(ui.painter(), Icon::Monitor, logo.center(), 13.0, Color32::WHITE);

                    ui.label(
                        RichText::new("RDP 管理器")
                            .size(14.0)
                            .strong()
                            .color(TEXT),
                    );

                    ui.add_space(4.0);
                    let (badge, _) =
                        ui.allocate_exact_size(vec2(66.0, 20.0), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(badge, CornerRadius::same(10), BG_ACTIVE);
                    ui.painter().text(
                        badge.center(),
                        Align2::CENTER_CENTER,
                        format!("{count} 个连接"),
                        FontId::proportional(11.0),
                        TEXT_MUTED,
                    );

                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            ui.spacing_mut().item_spacing.x = 10.0;

                            if ui
                                .add(
                                    ModernButton::new("新增连接")
                                        .icon(Icon::Plus)
                                        .kind(BtnKind::Primary)
                                        .min_w(116.0),
                                )
                                .clicked()
                            {
                                clicked_new = true;
                            }
                            if ui
                                .add(
                                    ModernButton::new("导入 .rdp")
                                        .icon(Icon::Import)
                                        .min_w(112.0),
                                )
                                .clicked()
                            {
                                clicked_import = true;
                            }
                            if master_enabled
                                && ui
                                    .add(
                                        ModernButton::new("锁定")
                                            .icon(Icon::Lock)
                                            .kind(BtnKind::Ghost)
                                            .min_w(78.0),
                                    )
                                    .clicked()
                            {
                                clicked_lock = true;
                            }
                            if ui
                                .add(
                                    ModernButton::new("主密码")
                                        .icon(Icon::Lock)
                                        .min_w(94.0),
                                )
                                .clicked()
                            {
                                clicked_master = true;
                            }

                            ui.add_space(6.0);
                            let edit = egui::TextEdit::singleline(&mut filter)
                                .frame(
                                    egui::Frame::new()
                                        .fill(BG_APP)
                                        .stroke(Stroke::new(1.0, BORDER_STRONG))
                                        .corner_radius(CornerRadius::same(6))
                                        .inner_margin(Margin {
                                            left: 32,
                                            right: 11,
                                            top: 7,
                                            bottom: 7,
                                        }),
                                )
                                .hint_text("搜索名称 / 地址")
                                .text_color(TEXT)
                                // 与左侧留给图标、右侧内边距的 frame 边距一致。
                                .margin(Margin {
                                    left: 32,
                                    right: 11,
                                    top: 0,
                                    bottom: 0,
                                });
                            let resp = ui.add_sized([220.0, 32.0], edit);
                            paint_icon(
                                ui.painter(),
                                Icon::Search,
                                pos2(resp.rect.min.x + 16.0, resp.rect.center().y),
                                14.0,
                                TEXT_FAINT,
                            );
                            if resp.has_focus() {
                                ui.painter().rect_stroke(
                                    resp.rect,
                                    CornerRadius::same(6),
                                    Stroke::new(1.0, ACCENT),
                                    StrokeKind::Inside,
                                );
                            }
                        },
                    );
                });
            });

        self.filter = filter;
        if clicked_new {
            self.open_new_form();
        }
        if clicked_import {
            self.pick_files();
        }
        if clicked_master {
            self.open_master_modal();
        }
        if clicked_lock {
            self.lock_now();
        }
    }

    // ── 状态栏 ──────────────────────────────────────────────────────
    fn draw_status_bar(&mut self, ui: &mut egui::Ui) {
        let status = self
            .status
            .as_ref()
            .map(|(m, e, _)| (m.clone(), *e));
        let count = self.store.entries.len();

        egui::Panel::bottom(egui::Id::new("app_status"))
            .frame(
                egui::Frame::new()
                    .fill(BG_SURFACE)
                    .inner_margin(Margin::symmetric(24, 9)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if let Some((msg, is_error)) = &status {
                        let color = if *is_error { DANGER } else { SUCCESS };
                        let (dot, _) =
                            ui.allocate_exact_size(vec2(8.0, 8.0), egui::Sense::hover());
                        ui.painter().circle_filled(dot.center(), 4.0, color);
                        ui.add_space(2.0);
                        ui.label(RichText::new(msg).size(12.5).color(TEXT));
                    } else {
                        ui.label(
                            RichText::new(
                                "双击任意一行启动连接 · 把 .rdp 文件拖进窗口即可导入",
                            )
                            .size(12.0)
                            .color(TEXT_FAINT),
                        );
                    }

                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            ui.label(
                                RichText::new(format!("共 {count} 个连接"))
                                    .size(12.0)
                                    .color(TEXT_FAINT),
                            );
                        },
                    );
                });
            });
    }

    // ── 主内容区 ────────────────────────────────────────────────────
    fn draw_content(&mut self, ui: &mut egui::Ui) {
        let total = self.store.entries.len();

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(BG_APP)
                    .inner_margin(Margin::same(24)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("连接列表")
                            .size(16.0)
                            .strong()
                            .color(TEXT),
                    );
                    ui.add_space(8.0);
                    let (badge, _) =
                        ui.allocate_exact_size(vec2(30.0, 20.0), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(badge, CornerRadius::same(10), BG_ELEVATED);
                    ui.painter().text(
                        badge.center(),
                        Align2::CENTER_CENTER,
                        total.to_string(),
                        FontId::proportional(11.0),
                        TEXT_MUTED,
                    );

                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            ui.label(
                                RichText::new("双击任意一行启动连接")
                                    .size(12.0)
                                    .color(TEXT_FAINT),
                            );
                        },
                    );
                });

                ui.add_space(16.0);
                self.draw_table(ui);
            });
    }

    #[allow(clippy::too_many_lines)]
    fn draw_table(&mut self, ui: &mut egui::Ui) {
        let filter = self.filter.trim().to_ascii_lowercase();
        let entries: Vec<Entry> = self
            .store
            .entries
            .iter()
            .filter(|e| {
                filter.is_empty()
                    || e.name.to_ascii_lowercase().contains(&filter)
                    || e.host.to_ascii_lowercase().contains(&filter)
            })
            .cloned()
            .collect();
        let total = entries.len();
        let selected = self.selected.clone();
        let interactive = !self.show_new && self.editing_id.is_none() && self.confirm_delete.is_none();
        let hover_pos = if interactive {
            ui.ctx().pointer_hover_pos()
        } else {
            None
        };

        let mut pending: Option<(String, RowAction)> = None;
        let mut next_selection: Option<String> = None;
        let mut empty_add = false;

        let card = egui::Frame::new()
            .fill(BG_SURFACE)
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin {
                left: 1,
                right: 1,
                top: 0,
                bottom: 1,
            });

        card.show(ui, |ui| {
            let full_w = ui.available_width();
            let cols = Columns::new(full_w);

            // 表头
            let (hr, _) =
                ui.allocate_exact_size(vec2(full_w, HEADER_H), egui::Sense::hover());
            ui.painter().rect_filled(
                hr,
                CornerRadius {
                    nw: 9,
                    ne: 9,
                    sw: 0,
                    se: 0,
                },
                BG_SURFACE_2,
            );
            ui.painter()
                .hline(hr.x_range(), hr.max.y - 0.5, Stroke::new(1.0, BORDER));

            let f = FontId::proportional(12.0);
            let cy = hr.center().y;
            let ox = hr.min.x;
            ui.painter().text(
                pos2(ox + cols.x_name, cy),
                Align2::LEFT_CENTER,
                "名称",
                f.clone(),
                TEXT_MUTED,
            );
            ui.painter().text(
                pos2(ox + cols.x_host, cy),
                Align2::LEFT_CENTER,
                "地址",
                f.clone(),
                TEXT_MUTED,
            );
            ui.painter().text(
                pos2(ox + cols.x_user, cy),
                Align2::LEFT_CENTER,
                "用户名",
                f.clone(),
                TEXT_MUTED,
            );
            ui.painter().text(
                pos2(hr.max.x - cols.pad, cy),
                Align2::RIGHT_CENTER,
                "操作",
                f,
                TEXT_MUTED,
            );

            if total == 0 {
                let (er, _) =
                    ui.allocate_exact_size(vec2(full_w, 230.0), egui::Sense::hover());
                let icon_c = pos2(er.center().x, er.min.y + 68.0);
                ui.painter().circle_filled(icon_c, 32.0, BG_ELEVATED);
                paint_icon(ui.painter(), Icon::Monitor, icon_c, 30.0, TEXT_FAINT);
                ui.painter().text(
                    pos2(er.center().x, er.min.y + 124.0),
                    Align2::CENTER_CENTER,
                    "还没有任何连接",
                    FontId::proportional(16.0),
                    TEXT,
                );
                ui.painter().text(
                    pos2(er.center().x, er.min.y + 150.0),
                    Align2::CENTER_CENTER,
                    "点「新增连接」创建，或把 .rdp 文件拖进窗口导入",
                    FontId::proportional(12.5),
                    TEXT_MUTED,
                );
                let bw = 132.0;
                let br = egui::Rect::from_center_size(
                    pos2(er.center().x, er.min.y + 192.0),
                    vec2(bw, 36.0),
                );
                if ui
                    .put(
                        br,
                        ModernButton::new("新增连接")
                            .icon(Icon::Plus)
                            .kind(BtnKind::Primary)
                            .min_w(bw)
                            .height(36.0),
                    )
                    .clicked()
                {
                    empty_add = true;
                }
                return;
            }

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (idx, entry) in entries.iter().enumerate() {
                        let row_rect = egui::Rect::from_min_size(
                            ui.available_rect_before_wrap().min,
                            vec2(full_w, ROW_H),
                        );
                        let is_selected = selected.as_deref() == Some(entry.id.as_str());
                        let hover = hover_pos.is_some_and(|p| row_rect.contains(p));
                        let is_last = idx + 1 == total;
                        let radius = if is_last {
                            CornerRadius {
                                nw: 0,
                                ne: 0,
                                sw: 11,
                                se: 11,
                            }
                        } else {
                            CornerRadius::ZERO
                        };

                        let bg = if is_selected {
                            ACCENT_SOFT
                        } else if hover {
                            BG_HOVER
                        } else if idx % 2 == 1 {
                            BG_SURFACE_2
                        } else {
                            BG_SURFACE
                        };
                        ui.painter().rect_filled(row_rect, radius, bg);
                        if is_selected {
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(row_rect.min, vec2(3.0, ROW_H)),
                                radius,
                                ACCENT,
                            );
                        }
                        if !is_last {
                            ui.painter().hline(
                                row_rect.x_range(),
                                row_rect.max.y - 0.5,
                                Stroke::new(
                                    1.0,
                                    Color32::from_rgba_unmultiplied_const(255, 255, 255, 8),
                                ),
                            );
                        }

                        let mut action: Option<RowAction> = None;
                        let inner = ui.scope_builder(
                            egui::UiBuilder::new()
                                .id_salt(("rdp_row", entry.id.as_str()))
                                .max_rect(row_rect)
                                .sense(egui::Sense::click())
                                .layout(egui::Layout::left_to_right(egui::Align::Center)),
                            |ui| {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                ui.set_min_height(ROW_H);
                                ui.add_space(cols.pad);

                                let (av, _) = ui.allocate_exact_size(
                                    vec2(30.0, 30.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().rect_filled(
                                    av,
                                    CornerRadius::same(9),
                                    avatar_color(&entry.name),
                                );
                                let ch = entry.name.chars().next().unwrap_or('?').to_string();
                                ui.painter().text(
                                    av.center(),
                                    Align2::CENTER_CENTER,
                                    ch,
                                    FontId::proportional(13.0),
                                    Color32::WHITE,
                                );
                                ui.add_space(cols.icon - 30.0);

                                ui.add_sized(
                                    [cols.name, ROW_H],
                                    egui::Label::new(
                                        RichText::new(clip(&entry.name, 24))
                                            .size(13.5)
                                            .strong()
                                            .color(TEXT),
                                    )
                                    .truncate(),
                                );
                                ui.add_sized(
                                    [cols.host, ROW_H],
                                    egui::Label::new(
                                        RichText::new(clip(&entry.host, 38))
                                            .monospace()
                                            .size(12.5)
                                            .color(TEXT_MUTED),
                                    )
                                    .truncate(),
                                );
                                ui.add_sized(
                                    [cols.user, ROW_H],
                                    egui::Label::new(
                                        RichText::new(clip(&entry.username, 20))
                                            .size(12.5)
                                            .color(TEXT_MUTED),
                                    )
                                    .truncate(),
                                );

                                let btn_total = 76.0 + 6.0 + 30.0 + 6.0 + 30.0;
                                let gap =
                                    (cols.actions - btn_total - cols.pad).max(0.0);
                                ui.add_space(gap);
                                if ui
                                    .add(
                                        ModernButton::new("连接")
                                            .icon(Icon::Play)
                                            .kind(BtnKind::Ghost)
                                            .min_w(76.0)
                                            .height(30.0),
                                    )
                                    .clicked()
                                {
                                    action = Some(RowAction::Connect);
                                }
                                ui.add_space(6.0);
                                if ui
                                    .add(IconButton {
                                        icon: Icon::Edit,
                                        danger: false,
                                        tip: "编辑连接",
                                    })
                                    .clicked()
                                {
                                    action = Some(RowAction::Edit);
                                }
                                ui.add_space(6.0);
                                if ui
                                    .add(IconButton {
                                        icon: Icon::Trash,
                                        danger: true,
                                        tip: "删除连接",
                                    })
                                    .clicked()
                                {
                                    action = Some(RowAction::Delete);
                                }
                                ui.add_space(cols.pad);
                            },
                        );

                        let response = inner.response;
                        if response.clicked() {
                            next_selection = Some(entry.id.clone());
                        }
                        if response.double_clicked() {
                            action = Some(RowAction::Connect);
                        }
                        if let Some(a) = action {
                            pending = Some((entry.id.clone(), a));
                        }
                    }
                });
        });

        if empty_add {
            self.open_new_form();
        }
        if let Some(id) = next_selection {
            self.selected = Some(id);
        }
        if let Some((id, act)) = pending {
            let entry = self.store.entries.iter().find(|e| e.id == id).cloned();
            if let Some(entry) = entry {
                match act {
                    RowAction::Connect => self.launch(&entry),
                    RowAction::Edit => self.open_edit_form(&entry),
                    RowAction::Delete => self.confirm_delete = Some(id),
                }
            }
        }
    }

    // ── 弹窗 ────────────────────────────────────────────────────────
    fn draw_new_modal(&mut self, ctx: &egui::Context) {
        if !self.show_new {
            return;
        }

        if self.form_focus_pending {
            ctx.memory_mut(|m| m.request_focus(egui::Id::new("form_name")));
            self.form_focus_pending = false;
        }

        let mut submit = false;
        let mut cancel = false;

        let modal = egui::Modal::new(egui::Id::new("new_connection_modal"))
            .backdrop_color(Color32::from_rgba_unmultiplied_const(6, 9, 16, 175))
            .frame(modal_frame());

        let resp = modal.show(ctx, |ui| {
            ui.set_width(340.0);
            ui.set_max_height(360.0);
            ui.spacing_mut().item_spacing.y = 0.0;

            ui.horizontal(|ui| {
                let (tile, _) =
                    ui.allocate_exact_size(vec2(32.0, 32.0), egui::Sense::hover());
                ui.painter()
                    .rect_filled(tile, CornerRadius::same(9), ACCENT_SOFT);
                paint_icon(ui.painter(), Icon::Plus, tile.center(), 16.0, ACCENT);

                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("新增连接")
                            .size(15.5)
                            .strong()
                            .color(TEXT),
                    );
                    ui.label(
                        RichText::new("创建一个新的远程桌面连接")
                            .size(11.0)
                            .color(TEXT_FAINT),
                    );
                });

                ui.with_layout(
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        if ui
                            .add(IconButton {
                                icon: Icon::Close,
                                danger: false,
                                tip: "关闭",
                            })
                            .clicked()
                        {
                            cancel = true;
                        }
                    },
                );
            });

            ui.add_space(14.0);
            form_field(
                ui,
                "名称",
                "留空则使用地址作为名称",
                &mut self.form_name,
                egui::Id::new("form_name"),
            );
            ui.add_space(10.0);
            form_field(
                ui,
                "地址",
                "192.168.1.10 或 host:3389",
                &mut self.form_host,
                egui::Id::new("form_host"),
            );
            ui.add_space(10.0);
            form_field(
                ui,
                "用户名",
                "可留空",
                &mut self.form_user,
                egui::Id::new("form_user"),
            );
            ui.add_space(10.0);
            form_field_password(
                ui,
                "密码",
                "可留空，连接时自动填入",
                &mut self.form_pass,
                egui::Id::new("form_pass"),
            );

            if let Some(err) = &self.form_error {
                ui.add_space(10.0);
                egui::Frame::new()
                    .fill(DANGER_SOFT)
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::symmetric(10, 7))
                    .show(ui, |ui| {
                        ui.label(RichText::new(err).size(12.0).color(DANGER));
                    });
            }

            ui.add_space(16.0);
            ui.with_layout(
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    if ui
                        .add(
                            ModernButton::new("创建连接")
                                .kind(BtnKind::Primary)
                                .min_w(96.0)
                                .height(32.0),
                        )
                        .clicked()
                    {
                        submit = true;
                    }
                    ui.add_space(8.0);
                    if ui
                        .add(
                            ModernButton::new("取消")
                                .kind(BtnKind::Ghost)
                                .min_w(68.0)
                                .height(32.0),
                        )
                        .clicked()
                    {
                        cancel = true;
                    }
                },
            );

            ui.add_space(2.0);
            ui.label(
                RichText::new("保存后会在数据目录生成对应的 .rdp 文件")
                    .size(10.5)
                    .color(TEXT_FAINT),
            );
        });

        if resp.backdrop_response.clicked() {
            cancel = true;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            submit = true;
        }

        if submit {
            self.save_new();
        } else if cancel {
            self.show_new = false;
            self.form_error = None;
        }
    }

    fn draw_edit_modal(&mut self, ctx: &egui::Context) {
        if self.editing_id.is_none() {
            return;
        }

        if self.edit_focus_pending {
            ctx.memory_mut(|m| m.request_focus(egui::Id::new("edit_name")));
            self.edit_focus_pending = false;
        }

        let mut submit = false;
        let mut cancel = false;

        let modal = egui::Modal::new(egui::Id::new("edit_connection_modal"))
            .backdrop_color(Color32::from_rgba_unmultiplied_const(6, 9, 16, 175))
            .frame(modal_frame());

        let resp = modal.show(ctx, |ui| {
            ui.set_width(340.0);
            ui.set_max_height(420.0);
            ui.spacing_mut().item_spacing.y = 0.0;

            ui.horizontal(|ui| {
                let (tile, _) =
                    ui.allocate_exact_size(vec2(32.0, 32.0), egui::Sense::hover());
                ui.painter()
                    .rect_filled(tile, CornerRadius::same(9), ACCENT_SOFT);
                paint_icon(ui.painter(), Icon::Edit, tile.center(), 16.0, ACCENT);

                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("编辑连接")
                            .size(15.5)
                            .strong()
                            .color(TEXT),
                    );
                    ui.label(
                        RichText::new("修改连接信息")
                            .size(11.0)
                            .color(TEXT_FAINT),
                    );
                });

                ui.with_layout(
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        if ui
                            .add(IconButton {
                                icon: Icon::Close,
                                danger: false,
                                tip: "关闭",
                            })
                            .clicked()
                        {
                            cancel = true;
                        }
                    },
                );
            });

            ui.add_space(14.0);
            form_field(
                ui,
                "名称",
                "留空则使用地址作为名称",
                &mut self.edit_name,
                egui::Id::new("edit_name"),
            );
            ui.add_space(10.0);
            form_field(
                ui,
                "地址",
                "192.168.1.10 或 host:3389",
                &mut self.edit_host,
                egui::Id::new("edit_host"),
            );
            ui.add_space(10.0);
            form_field(
                ui,
                "用户名",
                "可留空",
                &mut self.edit_user,
                egui::Id::new("edit_user"),
            );
            ui.add_space(10.0);
            form_field_password(
                ui,
                "密码",
                "可留空，连接时自动填入",
                &mut self.edit_pass,
                egui::Id::new("edit_pass"),
            );

            if let Some(err) = &self.edit_error {
                ui.add_space(10.0);
                egui::Frame::new()
                    .fill(DANGER_SOFT)
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::symmetric(10, 7))
                    .show(ui, |ui| {
                        ui.label(RichText::new(err).size(12.0).color(DANGER));
                    });
            }

            ui.add_space(16.0);
            ui.with_layout(
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    if ui
                        .add(
                            ModernButton::new("保存")
                                .kind(BtnKind::Primary)
                                .min_w(96.0)
                                .height(32.0),
                        )
                        .clicked()
                    {
                        submit = true;
                    }
                    ui.add_space(8.0);
                    if ui
                        .add(
                            ModernButton::new("取消")
                                .kind(BtnKind::Ghost)
                                .min_w(68.0)
                                .height(32.0),
                        )
                        .clicked()
                    {
                        cancel = true;
                    }
                },
            );
        });

        if resp.backdrop_response.clicked() {
            cancel = true;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            submit = true;
        }

        if submit {
            self.save_edit();
        } else if cancel {
            self.editing_id = None;
            self.edit_error = None;
        }
    }

    fn draw_delete_modal(&mut self, ctx: &egui::Context) {
        let Some(id) = self.confirm_delete.clone() else {
            return;
        };
        let name = self
            .store
            .entries
            .iter()
            .find(|e| e.id == id)
            .map(|e| e.name.clone())
            .unwrap_or_default();

        let mut confirmed = false;
        let mut cancelled = false;

        let modal = egui::Modal::new(egui::Id::new("confirm_delete_modal"))
            .backdrop_color(Color32::from_rgba_unmultiplied_const(6, 9, 16, 175))
            .frame(modal_frame());

        let resp = modal.show(ctx, |ui| {
            ui.set_width(310.0);

            let (tile, _) = ui.allocate_exact_size(vec2(36.0, 36.0), egui::Sense::hover());
            ui.painter()
                .rect_filled(tile, CornerRadius::same(10), DANGER_SOFT);
            paint_icon(ui.painter(), Icon::Trash, tile.center(), 18.0, DANGER);

            ui.add_space(12.0);
            ui.label(
                RichText::new("删除连接？")
                    .size(15.5)
                    .strong()
                    .color(TEXT),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new(format!(
                    "「{name}」将从列表中移除，对应的 .rdp 文件也会被删除。此操作无法撤销。"
                ))
                .size(12.5)
                .color(TEXT_MUTED),
            );

            ui.add_space(16.0);
            ui.with_layout(
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    if ui
                        .add(
                            ModernButton::new("删除")
                                .kind(BtnKind::DangerSolid)
                                .min_w(80.0)
                                .height(32.0),
                        )
                        .clicked()
                    {
                        confirmed = true;
                    }
                    ui.add_space(8.0);
                    if ui
                        .add(ModernButton::new("取消").min_w(72.0).height(32.0))
                        .clicked()
                    {
                        cancelled = true;
                    }
                },
            );
        });

        if resp.backdrop_response.clicked()
            || ctx.input(|i| i.key_pressed(egui::Key::Escape))
        {
            cancelled = true;
        }

        if confirmed {
            self.delete(&id);
            self.confirm_delete = None;
        } else if cancelled {
            self.confirm_delete = None;
        }
    }

    // ── 拖放覆盖层 ──────────────────────────────────────────────────
    fn draw_drop_overlay(ctx: &egui::Context) {
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("rdp_drop_overlay"),
        ));
        let rect = ctx.viewport_rect();
        painter.rect_filled(
            rect,
            CornerRadius::ZERO,
            Color32::from_rgba_unmultiplied_const(8, 13, 26, 205),
        );

        // 虚线放置区域
        let b = rect.shrink(24.0);
        let dash = 12.0;
        let gap = 8.0;
        let st = Stroke::new(2.0, ACCENT_DASH);
        let mut x = b.min.x + 6.0;
        while x < b.max.x - dash {
            painter.line_segment([pos2(x, b.min.y), pos2(x + dash, b.min.y)], st);
            painter.line_segment([pos2(x, b.max.y), pos2(x + dash, b.max.y)], st);
            x += dash + gap;
        }
        let mut y = b.min.y + 6.0;
        while y < b.max.y - dash {
            painter.line_segment([pos2(b.min.x, y), pos2(b.min.x, y + dash)], st);
            painter.line_segment([pos2(b.max.x, y), pos2(b.max.x, y + dash)], st);
            y += dash + gap;
        }

        let card = egui::Rect::from_center_size(rect.center(), vec2(360.0, 168.0));
        painter.rect_filled(card, CornerRadius::same(18), BG_SURFACE);
        painter.rect_stroke(
            card,
            CornerRadius::same(18),
            Stroke::new(1.0, BORDER_STRONG),
            StrokeKind::Inside,
        );
        paint_icon(
            &painter,
            Icon::Upload,
            pos2(card.center().x, card.min.y + 52.0),
            38.0,
            ACCENT,
        );
        painter.text(
            pos2(card.center().x, card.min.y + 104.0),
            Align2::CENTER_CENTER,
            "松开鼠标以导入 .rdp 文件",
            FontId::proportional(17.0),
            TEXT,
        );
        painter.text(
            pos2(card.center().x, card.min.y + 132.0),
            Align2::CENTER_CENTER,
            "支持一次拖入多个文件",
            FontId::proportional(12.5),
            TEXT_MUTED,
        );
    }

    // ── 主密码锁屏界面 ──────────────────────────────────────────────
    fn draw_lock_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let is_setup = self.auth.needs_initial_setup();
        if self.lock_focus_pending {
            ctx.memory_mut(|m| m.request_focus(egui::Id::new("lock_pass_input")));
            self.lock_focus_pending = false;
        }

        let mut submit = false;
        let mut skip_setup = false;

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG_APP))
            .show(ui, |ui| {
                let r = ui.max_rect();
                ui.painter().circle_filled(
                    pos2(r.center().x, r.min.y),
                    240.0,
                    Color32::from_rgba_unmultiplied_const(0x5B, 0x8C, 0xFF, 16),
                );
            });

        egui::Area::new(egui::Id::new("lock_card_area"))
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(BG_ELEVATED)
                    .stroke(Stroke::new(1.0, BORDER))
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(Margin {
                        left: 30,
                        right: 30,
                        top: 34,
                        bottom: 24,
                    })
                    .shadow(egui::Shadow {
                        offset: [0, 8],
                        blur: 28,
                        spread: 0,
                        color: Color32::from_rgba_unmultiplied_const(0, 0, 0, 97),
                    })
                    .show(ui, |ui| {
                        ui.set_width(320.0);
                        ui.spacing_mut().item_spacing.y = 0.0;

                        ui.vertical_centered(|ui| {
                            let (tile, _) =
                                ui.allocate_exact_size(vec2(46.0, 46.0), egui::Sense::hover());
                            ui.painter()
                                .rect_filled(tile, CornerRadius::same(14), ACCENT);
                            paint_icon(
                                ui.painter(),
                                Icon::Lock,
                                tile.center(),
                                20.0,
                                Color32::WHITE,
                            );

                            ui.add_space(16.0);
                            ui.label(
                                RichText::new("RDP 管理器")
                                    .size(20.0)
                                    .strong()
                                    .color(TEXT),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(if is_setup {
                                    "首次使用，请设置主密码"
                                } else {
                                    "输入主密码解锁（10 分钟无操作自动锁定）"
                                })
                                .size(12.0)
                                .color(TEXT_MUTED),
                            );
                        });

                        ui.add_space(20.0);
                        form_field_password(
                            ui,
                            "主密码",
                            "请输入主密码",
                            &mut self.lock_pass,
                            egui::Id::new("lock_pass_input"),
                        );

                        if is_setup {
                            ui.add_space(12.0);
                            form_field_password(
                                ui,
                                "确认主密码",
                                "再次输入主密码",
                                &mut self.lock_confirm,
                                egui::Id::new("lock_confirm_input"),
                            );
                        }

                        if let Some(err) = &self.lock_error {
                            ui.add_space(10.0);
                            ui.label(RichText::new(err).size(12.0).color(DANGER));
                        }

                        ui.add_space(14.0);
                        if ui
                            .add(
                                ModernButton::new(if is_setup { "设置并进入" } else { "解锁" })
                                    .kind(BtnKind::Primary)
                                    .min_w(320.0)
                                    .height(36.0),
                            )
                            .clicked()
                        {
                            submit = true;
                        }

                        if is_setup {
                            ui.add_space(10.0);
                            ui.vertical_centered(|ui| {
                                if ui
                                    .add(
                                        ModernButton::new("暂不启用")
                                            .kind(BtnKind::Ghost)
                                            .min_w(90.0)
                                            .height(30.0),
                                    )
                                    .clicked()
                                {
                                    skip_setup = true;
                                }
                            });
                        }
                    });
            });

        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            submit = true;
        }

        if skip_setup {
            match self.auth.disable() {
                Ok(()) => {
                    self.unlocked = true;
                    self.last_activity = Instant::now();
                    self.lock_pass.clear();
                    self.lock_confirm.clear();
                    self.lock_error = None;
                }
                Err(e) => self.lock_error = Some(e),
            }
        } else if submit {
            if is_setup {
                if self.lock_pass.is_empty() {
                    self.lock_error = Some("主密码不能为空".to_string());
                } else if self.lock_pass != self.lock_confirm {
                    self.lock_error = Some("两次输入的主密码不一致".to_string());
                } else {
                    match self.auth.set_password(&self.lock_pass) {
                        Ok(()) => {
                            self.unlocked = true;
                            self.last_activity = Instant::now();
                            self.lock_pass.clear();
                            self.lock_confirm.clear();
                            self.lock_error = None;
                            self.notify("已启用启动主密码", false);
                        }
                        Err(e) => self.lock_error = Some(e),
                    }
                }
            } else if self.auth.verify(&self.lock_pass) {
                self.unlocked = true;
                self.last_activity = Instant::now();
                self.lock_pass.clear();
                self.lock_error = None;
            } else {
                self.lock_error = Some("主密码错误，请重试".to_string());
                self.lock_pass.clear();
                self.lock_focus_pending = true;
            }
        }
    }

    // ── 主密码设置弹窗 ──────────────────────────────────────────────
    fn draw_master_modal(&mut self, ctx: &egui::Context) {
        if !self.show_master_modal {
            return;
        }

        let enabled = self.auth.is_enabled();
        if self.master_focus_pending {
            let id = if enabled {
                egui::Id::new("master_old")
            } else {
                egui::Id::new("master_new")
            };
            ctx.memory_mut(|m| m.request_focus(id));
            self.master_focus_pending = false;
        }

        let mut submit = false;
        let mut disable_auth = false;
        let mut cancel = false;

        let modal = egui::Modal::new(egui::Id::new("master_password_modal"))
            .backdrop_color(Color32::from_rgba_unmultiplied_const(6, 9, 16, 175))
            .frame(modal_frame());

        let resp = modal.show(ctx, |ui| {
            ui.set_width(340.0);
            ui.set_max_height(380.0);
            ui.spacing_mut().item_spacing.y = 0.0;

            ui.horizontal(|ui| {
                let (tile, _) =
                    ui.allocate_exact_size(vec2(32.0, 32.0), egui::Sense::hover());
                ui.painter()
                    .rect_filled(tile, CornerRadius::same(9), ACCENT_SOFT);
                paint_icon(ui.painter(), Icon::Lock, tile.center(), 16.0, ACCENT);

                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(if enabled {
                            "管理启动主密码"
                        } else {
                            "启用启动主密码"
                        })
                        .size(15.5)
                        .strong()
                        .color(TEXT),
                    );
                    ui.label(
                        RichText::new("启动或 10 分钟无操作时自动锁定")
                            .size(11.0)
                            .color(TEXT_FAINT),
                    );
                });

                ui.with_layout(
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        if ui
                            .add(IconButton {
                                icon: Icon::Close,
                                danger: false,
                                tip: "关闭",
                            })
                            .clicked()
                        {
                            cancel = true;
                        }
                    },
                );
            });

            ui.add_space(14.0);
            if enabled {
                form_field_password(
                    ui,
                    "当前主密码",
                    "请输入当前主密码",
                    &mut self.master_old,
                    egui::Id::new("master_old"),
                );
                ui.add_space(10.0);
            }
            form_field_password(
                ui,
                "新主密码",
                "请输入新主密码",
                &mut self.master_new,
                egui::Id::new("master_new"),
            );
            ui.add_space(10.0);
            form_field_password(
                ui,
                "确认新主密码",
                "再次输入新主密码",
                &mut self.master_confirm,
                egui::Id::new("master_confirm"),
            );

            if let Some(err) = &self.master_error {
                ui.add_space(10.0);
                egui::Frame::new()
                    .fill(DANGER_SOFT)
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::symmetric(10, 7))
                    .show(ui, |ui| {
                        ui.label(RichText::new(err).size(12.0).color(DANGER));
                    });
            }

            ui.add_space(16.0);
            ui.with_layout(
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    if ui
                        .add(
                            ModernButton::new(if enabled { "保存修改" } else { "启用主密码" })
                                .kind(BtnKind::Primary)
                                .min_w(98.0)
                                .height(32.0),
                        )
                        .clicked()
                    {
                        submit = true;
                    }
                    if enabled
                        && ui
                            .add(
                                ModernButton::new("关闭主密码")
                                    .kind(BtnKind::DangerSolid)
                                    .min_w(92.0)
                                    .height(32.0),
                            )
                            .clicked()
                    {
                        disable_auth = true;
                    }
                    if ui
                        .add(
                            ModernButton::new("取消")
                                .kind(BtnKind::Ghost)
                                .min_w(72.0)
                                .height(32.0),
                        )
                        .clicked()
                    {
                        cancel = true;
                    }
                },
            );
        });

        if resp.backdrop_response.clicked() || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            submit = true;
        }

        if cancel {
            self.show_master_modal = false;
            self.master_old.clear();
            self.master_new.clear();
            self.master_confirm.clear();
            self.master_error = None;
        } else if disable_auth {
            if !self.auth.verify(&self.master_old) {
                self.master_error = Some("当前主密码错误".to_string());
            } else {
                match self.auth.disable() {
                    Ok(()) => {
                        self.show_master_modal = false;
                        self.master_old.clear();
                        self.master_new.clear();
                        self.master_confirm.clear();
                        self.master_error = None;
                        self.notify("已关闭启动主密码", false);
                    }
                    Err(e) => self.master_error = Some(e),
                }
            }
        } else if submit {
            if enabled && !self.auth.verify(&self.master_old) {
                self.master_error = Some("当前主密码错误".to_string());
            } else if self.master_new.is_empty() {
                self.master_error = Some("新主密码不能为空".to_string());
            } else if self.master_new != self.master_confirm {
                self.master_error = Some("两次输入的新主密码不一致".to_string());
            } else {
                match self.auth.set_password(&self.master_new) {
                    Ok(()) => {
                        self.show_master_modal = false;
                        self.last_activity = Instant::now();
                        self.master_old.clear();
                        self.master_new.clear();
                        self.master_confirm.clear();
                        self.master_error = None;
                        self.notify(
                            if enabled {
                                "主密码已更新"
                            } else {
                                "已启用启动主密码"
                            },
                            false,
                        );
                    }
                    Err(e) => self.master_error = Some(e),
                }
            }
        }
    }

    // ── 退出确认弹窗 ────────────────────────────────────────────────
    fn draw_exit_modal(&mut self, ctx: &egui::Context) {
        if !self.confirm_exit {
            return;
        }

        let mut confirm = false;
        let mut cancel = false;

        let modal = egui::Modal::new(egui::Id::new("exit_confirm_modal"))
            .backdrop_color(Color32::from_rgba_unmultiplied_const(6, 9, 16, 175))
            .frame(modal_frame());

        let resp = modal.show(ctx, |ui| {
            ui.set_width(320.0);
            ui.spacing_mut().item_spacing.y = 0.0;

            ui.horizontal(|ui| {
                let (tile, _) =
                    ui.allocate_exact_size(vec2(32.0, 32.0), egui::Sense::hover());
                ui.painter()
                    .rect_filled(tile, CornerRadius::same(9), DANGER_SOFT);
                paint_icon(ui.painter(), Icon::Close, tile.center(), 15.0, DANGER);

                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("确认退出")
                            .size(15.5)
                            .strong()
                            .color(TEXT),
                    );
                    ui.label(
                        RichText::new("确定要退出 RDP 管理器吗？")
                            .size(12.0)
                            .color(TEXT_MUTED),
                    );
                });
            });

            ui.add_space(18.0);
            ui.with_layout(
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    if ui
                        .add(
                            ModernButton::new("取消")
                                .kind(BtnKind::Secondary)
                                .min_w(78.0)
                                .height(32.0),
                        )
                        .clicked()
                    {
                        cancel = true;
                    }
                    if ui
                        .add(
                            ModernButton::new("确定")
                                .kind(BtnKind::DangerSolid)
                                .min_w(78.0)
                                .height(32.0),
                        )
                        .clicked()
                    {
                        confirm = true;
                    }
                },
            );
        });

        if resp.backdrop_response.clicked() || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }

        if confirm {
            self.confirm_exit = false;
            self.tray.mark_quitting();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else if cancel {
            self.confirm_exit = false;
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        for ev in self.tray.poll() {
            match ev {
                tray::TrayEvent::Show => {
                    self.hidden_to_tray = false;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                tray::TrayEvent::Quit => {
                    self.tray.mark_quitting();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }

        if !self.hidden_to_tray && ctx.input(|i| i.viewport().minimized == Some(true)) {
            self.hidden_to_tray = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        if ctx.input(|i| i.viewport().close_requested()) && !self.tray.is_quitting() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm_exit = true;
            if self.hidden_to_tray {
                self.hidden_to_tray = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
        }

        if !self.unlocked {
            self.draw_lock_screen(ui, &ctx);
            self.draw_exit_modal(&ctx);
            return;
        }

        if self.auth.is_enabled() {
            let has_input = ctx.input(|i| !i.events.is_empty());
            if has_input {
                self.last_activity = Instant::now();
            } else if self.last_activity.elapsed() >= Duration::from_secs(AUTO_LOCK_SECS) {
                self.lock_now();
                self.draw_lock_screen(ui, &ctx);
                self.draw_exit_modal(&ctx);
                return;
            }
            ctx.request_repaint_after(Duration::from_secs(5));
        }

        let dropped: Vec<egui::DroppedFile> = ctx.input(|i| i.raw.dropped_files.clone());
        let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());
        if !dropped.is_empty() {
            self.handle_dropped(dropped);
        }

        if let Some((_, _, since)) = &self.status {
            if since.elapsed() > Duration::from_secs(8) {
                self.status = None;
            } else {
                ctx.request_repaint_after(Duration::from_secs(1));
            }
        }

        self.draw_header(ui);
        self.draw_status_bar(ui);
        self.draw_content(ui);

        self.draw_new_modal(&ctx);
        self.draw_edit_modal(&ctx);
        self.draw_delete_modal(&ctx);
        self.draw_master_modal(&ctx);
        self.draw_exit_modal(&ctx);

        if hovering {
            Self::draw_drop_overlay(&ctx);
        }
    }

    fn on_exit(&mut self) {
        self.tray.shutdown();
    }
}

fn clip(s: &str, max_chars: usize) -> String {
    if s.is_empty() {
        return "-".to_string();
    }
    let mut out: String = s.chars().take(max_chars).collect();
    if s.chars().count() > max_chars {
        out.push('…');
    }
    out
}

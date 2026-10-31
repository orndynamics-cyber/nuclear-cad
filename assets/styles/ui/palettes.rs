//! The built-in palettes: Nuclear Light (`light`) and Nuclear Graphite (`dark`), the design system every theme starts from and every path without a theme file falls back to. Compiled by the UI theme boundary.
use crate::theme::{AnalysisColors, IconColors, Tokens};
use egui::Color32;

const fn icon_primary(rgb: [u8; 3]) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}
impl Tokens {
    pub const fn light() -> Tokens {
        Tokens {
            nav_hover: Color32::from_rgb(230, 233, 237),
            nav_press: Color32::from_rgb(217, 221, 226),
            nav_dim: Color32::from_rgb(138, 146, 157),
            nav_viewport: Color32::from_rgb(241, 243, 245),
            nav_tip: Color32::from_rgb(35, 39, 45),
            nav_tip_ink: Color32::from_rgb(255, 255, 255),
            nav_tip_key_border: Color32::from_rgba_unmultiplied_const(128, 140, 160, 153),
            nav_field: Color32::from_rgb(255, 255, 255),

            borders: crate::theme::Borders::LEGACY,
            icon_palette: None,
            file_dim: Color32::from_rgb(58, 70, 86),
            file_recovery_well: Color32::from_rgb(247, 226, 184),
            file_action_height: 32.0,
            file_back_height: 40.0,
            file_recovery_icon: 32.0,
            file_grid_gap: 14.0,
            file_background: Color32::from_rgb(239, 237, 233),
            file_surface: Color32::WHITE,
            file_border: Color32::from_rgb(224, 229, 236),
            file_image_tint: Color32::WHITE,
            file_rail: Color32::from_rgb(230, 227, 222),
            file_rail_ink: Color32::from_rgb(32, 35, 39),
            file_rail_hover: Color32::from_rgb(216, 212, 205),
            file_primary: Color32::from_rgb(244, 225, 211),
            file_primary_ink: Color32::from_rgb(32, 35, 39),
            file_primary_border: Color32::from_rgb(184, 72, 15),
            file_recovery: Color32::from_rgb(255, 246, 229),
            file_recovery_ink: Color32::from_rgb(122, 74, 5),
            file_recovery_border: Color32::from_rgb(240, 210, 154),
            file_nav_width: 248.0,
            file_nav_height: 38.0,
            file_tile_height: 84.0,
            file_tile_icon: 44.0,
            file_recent_height: 52.0,
            file_sample_image_height: 84.0,
            file_content_inset_x: 40.0,
            file_content_inset_y: 32.0,
            file_section_gap: 28.0,
            font_start_title: 28.0,
            space_2: 2.0,
            space_3: 3.0,
            space_4: 4.0,
            space_6: 6.0,
            space_8: 8.0,
            space_10: 10.0,
            space_12: 12.0,
            space_14: 14.0,
            space_16: 16.0,
            space_24: 24.0,
            radius_sm: 4,
            radius_md: 6,
            radius_lg: 8,
            radius_xl: 10,
            font_badge: 9.5,
            font_micro: 10.0,
            font_panel: 10.5,
            font_small: 11.0,
            font_caption: 11.5,
            font_body: 12.0,
            font_label: 12.5,
            font_button: 13.0,
            font_heading: 14.0,
            font_triad: 8.0,
            font_compass: 13.5,
            font_subtitle: 15.0,
            font_intro: 16.0,
            font_article_heading: 17.0,
            font_title: 22.0,
            font_display: 26.0,
            control_height: 28.0,
            form_ink: Color32::from_rgb(32, 35, 39),
            form_dim: Color32::from_rgb(79, 85, 93),
            form_primary: Color32::from_rgb(184, 72, 15),
            form_primary_fill: Color32::from_rgb(184, 72, 15),
            form_error: Color32::from_rgb(199, 21, 41),
            form_error_fill: Color32::from_rgb(244, 224, 222),
            form_success: Color32::from_rgb(37, 114, 49),
            form_success_fill: Color32::from_rgb(228, 233, 223),
            control_radius: 6,
            form_label_width: 70.0,
            form_gap_y: 5.0,
            form_footer_height: 48.0,
            form_header_height: 40.0,
            form_card_width: 264.0,
            form_icon_size: 19.0,
            form_close_size: 26.0,
            form_inset: 9.0,
            form_icon_inset: 4.5,
            form_section_icon_size: 11.0,
            form_checkbox_height: 22.0,
            form_button_height: 28.0,
            sketch_palette_width: 232.0,
            sketch_mode_size: 36.0,
            sketch_mode_radius: 7,
            sketch_mode_border: 1.5,
            sketch_palette_alpha: 0.96,
            sketch_mode_tint: 0.12,
            sketch_footer_icon_size: 15.0,
            font_section_tracking: 0.04,
            app_bar: Color32::from_rgb(222, 219, 214),
            app_bar_separator: Color32::from_rgb(115, 108, 98),
            toolbar: Color32::from_rgb(239, 237, 233),
            panel: Color32::from_rgb(239, 237, 233),
            panel_header: Color32::from_rgb(230, 227, 222),
            border: Color32::from_rgb(196, 191, 183),
            text: Color32::from_rgb(32, 35, 39),
            text_dim: Color32::from_rgb(79, 85, 93),
            accent: Color32::from_rgb(184, 72, 15),
            accent_soft: Color32::from_rgb(244, 225, 211),
            sketch_accent: icon_primary(crate::icon_svg::SKETCH_PRIMARY[0]),
            // Icon palette active-3 ramps; these accents have no compiled constants yet.
            surface_accent: Color32::from_rgb(164, 80, 0),
            mesh_accent: Color32::from_rgb(130, 103, 0),
            form_accent: Color32::from_rgb(115, 73, 205),
            render_accent: Color32::from_rgb(174, 45, 113),
            sketch_ink: Color32::from_rgb(4, 112, 60),
            sketch_soft: Color32::from_rgb(237, 247, 241),
            surface_fill: Color32::from_rgb(143, 90, 0),
            surface_ink: Color32::from_rgb(127, 80, 0),
            surface_soft: Color32::from_rgb(246, 232, 210),
            mesh_fill: Color32::from_rgb(111, 100, 0),
            mesh_ink: Color32::from_rgb(98, 88, 0),
            mesh_soft: Color32::from_rgb(242, 239, 214),
            form_fill: Color32::from_rgb(122, 74, 168),
            form_tab_ink: Color32::from_rgb(110, 63, 156),
            form_soft: Color32::from_rgb(240, 232, 246),
            sheet_fill: Color32::from_rgb(86, 96, 107),
            sheet_ink: Color32::from_rgb(75, 84, 94),
            sheet_soft: Color32::from_rgb(230, 232, 235),
            render_fill: Color32::from_rgb(168, 58, 72),
            render_ink: Color32::from_rgb(156, 50, 65),
            render_soft: Color32::from_rgb(247, 226, 228),
            tab_on_fill_dark: Color32::from_rgb(20, 17, 13),
            on_accent: Color32::from_rgb(255, 255, 255),
            agent_ok: Color32::from_rgb(76, 194, 122),
            agent_busy: Color32::from_rgb(184, 72, 15),
            error: Color32::from_rgb(199, 21, 41),
            warning: Color32::from_rgb(222, 150, 30),
            icon: Color32::from_rgb(32, 35, 39),
            timeline: Color32::from_rgb(230, 227, 222),
            hover: Color32::from_rgb(216, 212, 205),
            sel_face: Color32::from_rgb(215, 75, 35),
            sel_edge: Color32::from_rgb(255, 122, 50),
            sel_edge_rim: Color32::from_rgb(255, 196, 160),
            hover_edge: Color32::from_rgb(0, 0, 0),
            hover_edge_halo: Color32::from_rgb(203, 203, 203),
            hover_face_lift: 40,
            sel_profile: Color32::from_rgb(26, 84, 167),
            sel_profile_edge: Color32::from_rgb(47, 109, 198),
            hover_profile: Color32::from_rgba_unmultiplied_const(75, 129, 253, 48),
            hover_profile_edge: Color32::from_rgb(75, 129, 253),
            sel_vertex: Color32::from_rgb(190, 84, 20),
            origin_plane: Color32::from_rgba_unmultiplied_const(249, 184, 134, 94),
            origin_plane_edge: Color32::from_rgba_unmultiplied_const(214, 140, 84, 200),
            origin_plane_hover: Color32::from_rgba_unmultiplied_const(120, 130, 230, 150),
            origin_point: Color32::from_rgb(214, 215, 218),
            // The one set of X, Y and Z colours (#976), equal to the engine's `view::colors::AXIS_*`
            // so a headless picture draws its origin axes as the window does.
            axis_x: Color32::from_rgb(220, 60, 60),
            axis_y: Color32::from_rgb(60, 170, 60),
            axis_z: Color32::from_rgb(60, 100, 230),
            construction_plane: Color32::from_rgba_unmultiplied_const(249, 184, 134, 70),
            construction_geom: icon_primary(crate::icon_svg::CONSTRUCTION_PRIMARY[0]),
            body: Color32::from_rgb(176, 186, 198),
            body_edge: Color32::from_rgb(40, 44, 52),
            manipulator: Color32::from_rgb(40, 110, 220),
            preview_add: Color32::from_rgb(112, 160, 226),
            preview_new: Color32::from_rgb(240, 225, 105),
            preview_sketch: Color32::from_rgb(32, 190, 96),
            preview_cut: Color32::from_rgba_unmultiplied_const(232, 84, 44, 110),
            cube_face: Color32::from_rgb(253, 250, 243),
            cube_border: Color32::from_rgb(183, 177, 167),
            cube_text: Color32::from_rgb(20, 17, 13),
            cube_hover: Color32::from_rgb(244, 225, 211),
            viewport_top: Color32::from_rgb(236, 234, 231),
            viewport_bottom: Color32::from_rgb(201, 203, 207),
            grid: Color32::from_rgb(217, 216, 213),
            grid_major: Color32::from_rgb(195, 195, 194),
            body_selected: Color32::from_rgb(235, 155, 95),
            sketch: Color32::from_rgb(30, 90, 200),
            sketch_fixed: Color32::from_rgb(20, 20, 20),
            sketch_halo: Color32::from_rgb(241, 243, 245),
            sketch_point: Color32::BLACK,
            sketch_locked: Color32::from_rgb(0, 140, 60),
            // The error red (`error`).
            sketch_conflict: Color32::from_rgb(199, 21, 41),
            // Sketch dimensions in a plum of their own, apart from the blue and black curves for
            // every colour vision mode (#7861).
            dimension: Color32::from_rgb(130, 42, 110),
            dimension_driven: Color32::from_rgb(160, 90, 120),
            sketch_point_free: Color32::from_rgb(30, 90, 200),
            section_cap: Color32::from_rgb(214, 128, 72),
            dialog_bg: Color32::from_rgb(239, 237, 233),
            dialog_border: Color32::from_rgb(196, 191, 183),
            chip: Color32::from_rgb(184, 72, 15),
            profile_fill: Color32::from_rgba_unmultiplied_const(255, 196, 120, 70),
            glyph_bg: Color32::WHITE,
            glyph_edge: Color32::from_rgb(190, 110, 100),
            glyph_text: icon_primary(crate::icon_svg::CONSTRAINT_PRIMARY[0]),
            overlay: Color32::from_rgb(239, 237, 233),
            rubber_band: Color32::from_rgb(30, 120, 230),
            app_bar_text: Color32::from_rgb(32, 35, 39),
            app_bar_hover: Color32::from_rgba_unmultiplied_const(20, 17, 13, 20),
            app_bar_hover_subtle: Color32::from_rgba_unmultiplied_const(20, 17, 13, 12),
            app_bar_hover_button: Color32::from_rgba_unmultiplied_const(20, 17, 13, 24),
            app_bar_hover_strong: Color32::from_rgba_unmultiplied_const(20, 17, 13, 34),
            app_bar_tab_active: Color32::from_rgb(239, 237, 233),
            dialog_body: Color32::from_rgb(239, 237, 233),
            dialog_footer: Color32::from_rgb(239, 237, 233),
            dialog_rule: Color32::from_rgb(196, 191, 183),
            control_fill: Color32::from_rgb(250, 249, 247),
            control_border: Color32::from_rgb(196, 191, 183),
            dialog_label: Color32::from_rgb(32, 35, 39),
            selected_ink: Color32::from_rgb(42, 37, 31),
            selected_bg: Color32::from_rgb(242, 217, 200),
            timeline_item: Color32::from_rgb(250, 249, 247),
            timeline_item_error: Color32::from_rgb(250, 220, 220),
            timeline_marker: Color32::from_rgb(191, 86, 52),
            field: Color32::from_rgb(250, 249, 247),
            box_window: Color32::from_rgb(38, 120, 218),
            box_crossing: Color32::from_rgb(40, 160, 80),
            // Equal to `view::colors` (the engine's tags), so mapping a tag to its token
            // changes nothing.
            sketch_projected: Color32::from_rgb(150, 60, 190),
            sketch_lost: Color32::from_rgb(200, 60, 90),
            sketch_construction: Color32::from_rgb(230, 130, 40),
            sketch_centerline: Color32::from_rgb(200, 110, 60),
            bend_up: Color32::from_rgb(30, 140, 50),
            bend_down: Color32::from_rgb(200, 90, 20),
            interference: Color32::from_rgb(235, 45, 40),
            continuity: [
                Color32::from_rgb(0, 158, 115),
                Color32::from_rgb(0, 114, 178),
                Color32::from_rgb(240, 200, 0),
                Color32::from_rgb(204, 0, 140),
            ],
            analysis: AnalysisColors::DEFAULT,
            icons: IconColors { highlight: Color32::WHITE, agent: Color32::from_rgb(168, 85, 247), marker_halo: Color32::from_black_alpha(150) },
            logo: [Color32::from_rgb(224, 122, 60), Color32::from_rgb(184, 72, 15), Color32::from_rgb(106, 38, 4)],
        }
    }
    pub const fn dark() -> Tokens {
        let mut t = Tokens::light();
        // Shared typography and dense form dimensions stay identical in both palettes.
        t.font_triad = 8.0;
        t.font_compass = 13.5;
        t.font_subtitle = 15.0;
        t.font_intro = 16.0;
        t.font_article_heading = 17.0;
        t.font_title = 22.0;
        t.font_display = 26.0;
        t.control_height = 28.0;
        t.control_radius = 6;
        t.form_label_width = 70.0;
        t.form_gap_y = 5.0;
        t.form_footer_height = 48.0;
        t.form_header_height = 40.0;
        t.form_card_width = 264.0;
        t.form_icon_size = 19.0;
        t.form_close_size = 26.0;
        t.form_inset = 9.0;
        t.form_icon_inset = 4.5;
        t.form_section_icon_size = 11.0;
        t.form_checkbox_height = 22.0;
        t.form_button_height = 28.0;
        t.sketch_palette_width = 232.0;
        t.sketch_mode_size = 36.0;
        t.sketch_mode_radius = 7;
        t.sketch_mode_border = 1.5;
        t.sketch_palette_alpha = 0.96;
        t.sketch_mode_tint = 0.12;
        t.sketch_footer_icon_size = 15.0;
        t.font_section_tracking = 0.04;
        t.file_dim = Color32::from_rgb(180, 187, 200);
        t.file_recovery_well = Color32::from_rgb(92, 68, 32);
        t.file_background = Color32::from_rgb(35, 39, 45);
        t.file_surface = Color32::from_rgb(39, 43, 52);
        t.file_border = Color32::from_rgb(63, 69, 82);
        t.file_rail = Color32::from_rgb(35, 39, 45);
        t.file_rail_hover = Color32::from_rgb(57, 66, 77);
        t.file_primary = Color32::from_rgb(74, 48, 32);
        t.file_primary_ink = Color32::from_rgb(241, 243, 245);
        t.file_primary_border = Color32::from_rgb(255, 122, 50);
        t.file_recovery = Color32::from_rgb(67, 51, 28);
        t.file_recovery_ink = Color32::from_rgb(245, 204, 137);
        t.file_recovery_border = Color32::from_rgb(115, 88, 43);
        t.app_bar = Color32::from_rgb(25, 28, 32);
        t.toolbar = Color32::from_rgb(35, 39, 45);
        t.panel = Color32::from_rgb(35, 39, 45);
        t.panel_header = Color32::from_rgb(45, 51, 59);
        t.border = Color32::from_rgb(65, 74, 86);
        t.text = Color32::from_rgb(241, 243, 245);
        t.text_dim = Color32::from_rgb(184, 192, 203);
        t.accent = Color32::from_rgb(255, 122, 50);
        t.accent_soft = Color32::from_rgb(74, 48, 32);
        t.sketch_accent = Color32::from_rgb(18, 141, 69);
        t.surface_accent = Color32::from_rgb(149, 104, 45);
        t.mesh_accent = Color32::from_rgb(133, 113, 0);
        t.form_accent = Color32::from_rgb(123, 99, 166);
        t.render_accent = Color32::from_rgb(224, 116, 174);
        t.sketch_ink = Color32::from_rgb(70, 190, 120);
        t.sketch_soft = Color32::from_rgb(31, 59, 43);
        t.surface_fill = Color32::from_rgb(232, 160, 60);
        t.surface_ink = Color32::from_rgb(240, 180, 94);
        t.surface_soft = Color32::from_rgb(67, 53, 34);
        t.mesh_fill = Color32::from_rgb(201, 178, 74);
        t.mesh_ink = Color32::from_rgb(214, 194, 102);
        t.mesh_soft = Color32::from_rgb(58, 56, 36);
        t.form_fill = Color32::from_rgb(180, 138, 214);
        t.form_tab_ink = Color32::from_rgb(201, 168, 232);
        t.form_soft = Color32::from_rgb(58, 47, 69);
        t.sheet_fill = Color32::from_rgb(169, 179, 190);
        t.sheet_ink = Color32::from_rgb(194, 202, 211);
        t.sheet_soft = Color32::from_rgb(50, 56, 64);
        t.render_fill = Color32::from_rgb(224, 112, 122);
        t.render_ink = Color32::from_rgb(238, 144, 153);
        t.render_soft = Color32::from_rgb(70, 40, 44);
        t.tab_on_fill_dark = Color32::from_rgb(6, 7, 8);
        t.nav_hover = Color32::from_rgb(57, 66, 77);
        t.nav_press = Color32::from_rgb(65, 74, 86);
        t.nav_dim = Color32::from_rgb(138, 146, 157);
        t.nav_viewport = Color32::from_rgb(35, 39, 45);
        t.nav_tip = Color32::from_rgb(241, 243, 245);
        t.nav_tip_ink = Color32::from_rgb(25, 28, 32);
        t.nav_tip_key_border = Color32::from_rgba_unmultiplied_const(128, 140, 160, 153);
        t.nav_field = Color32::from_rgb(29, 33, 37);
        t.dialog_body = Color32::from_rgb(35, 39, 45);
        t.dialog_footer = Color32::from_rgb(35, 39, 45);
        t.dialog_rule = Color32::from_rgb(65, 74, 86);
        t.control_fill = Color32::from_rgb(45, 51, 59);
        t.control_border = Color32::from_rgb(65, 74, 86);
        t.dialog_label = Color32::from_rgb(241, 243, 245);
        t.selected_ink = Color32::from_rgb(241, 243, 245);
        t.selected_bg = Color32::from_rgb(90, 52, 24);
        t.construction_geom = Color32::from_rgb(167, 93, 0);
        t.error = Color32::from_rgb(240, 100, 96);
        t.sketch_conflict = t.error;
        t.warning = Color32::from_rgb(236, 170, 70);
        t.icon = Color32::from_rgb(241, 243, 245);
        t.timeline = Color32::from_rgb(45, 51, 59);
        t.hover = Color32::from_rgb(57, 66, 77);
        t.hover_edge_halo = Color32::from_rgb(203, 203, 203);
        t.origin_point = Color32::from_rgb(214, 215, 218);
        t.viewport_top = Color32::from_rgb(44, 49, 55);
        t.viewport_bottom = Color32::from_rgb(26, 29, 33);
        t.grid = Color32::from_rgb(52, 58, 65);
        t.grid_major = Color32::from_rgb(66, 73, 81);
        t.body = Color32::from_rgb(152, 156, 162);
        t.body_edge = Color32::from_rgb(16, 16, 18);
        t.sketch = Color32::from_rgb(92, 150, 255);
        t.sketch_fixed = Color32::from_rgb(232, 234, 238);
        t.sketch_halo = Color32::from_rgb(25, 28, 32);
        t.sketch_point = Color32::from_rgb(232, 234, 238);
        t.sketch_locked = Color32::from_rgb(96, 204, 116);
        // Sketch dimensions: Light's plum, lightened for the dark view (#7861).
        t.dimension = Color32::from_rgb(224, 176, 224);
        t.dimension_driven = Color32::from_rgb(176, 104, 144);
        t.sketch_point_free = Color32::from_rgb(92, 150, 255);
        t.profile_fill = Color32::from_rgba_unmultiplied_const(121, 153, 198, 40);
        t.dialog_bg = Color32::from_rgb(35, 39, 45);
        t.dialog_border = Color32::from_rgb(65, 74, 86);
        t.chip = Color32::from_rgb(255, 122, 50);
        t.glyph_bg = Color32::from_rgb(44, 50, 60);
        t.glyph_edge = Color32::from_rgb(200, 130, 120);
        t.glyph_text = Color32::from_rgb(193, 99, 88);
        t.overlay = Color32::from_rgb(35, 39, 45);
        t.rubber_band = Color32::from_rgb(92, 160, 255);
        t.timeline_item = Color32::from_rgb(45, 51, 59);
        t.timeline_item_error = Color32::from_rgb(110, 50, 54);
        t.timeline_marker = Color32::from_rgb(184, 192, 203);
        t.field = Color32::from_rgb(29, 33, 37);
        t.box_window = Color32::from_rgb(70, 150, 255);
        t.box_crossing = Color32::from_rgb(80, 200, 120);
        t.bend_up = Color32::from_rgb(110, 200, 120);
        t.bend_down = Color32::from_rgb(240, 150, 80);
        t.form_ink = Color32::from_rgb(241, 243, 245);
        t.form_dim = Color32::from_rgb(184, 192, 203);
        t.form_primary = Color32::from_rgb(255, 122, 50);
        t.form_primary_fill = Color32::from_rgb(255, 122, 50);
        t.form_error = t.error;
        t.form_error_fill = t.timeline_item_error;
        t.form_success = Color32::from_rgb(143, 219, 154);
        t.form_success_fill = Color32::from_rgb(44, 74, 51);
        // Nuclear Graphite where it differs from Nuclear Light (generated from the theme files).
        t.agent_busy = Color32::from_rgb(255, 122, 50);
        t.app_bar_hover = Color32::from_rgba_unmultiplied_const(241, 243, 245, 24);
        t.app_bar_hover_button = Color32::from_rgba_unmultiplied_const(241, 243, 245, 34);
        t.app_bar_hover_strong = Color32::from_rgba_unmultiplied_const(241, 243, 245, 48);
        t.app_bar_hover_subtle = Color32::from_rgba_unmultiplied_const(241, 243, 245, 16);
        t.app_bar_separator = Color32::from_rgb(100, 110, 123);
        t.app_bar_tab_active = Color32::from_rgb(45, 51, 59);
        t.app_bar_text = Color32::from_rgb(241, 243, 245);
        t.cube_border = Color32::from_rgb(65, 74, 86);
        t.cube_face = Color32::from_rgb(45, 51, 59);
        t.cube_hover = Color32::from_rgb(57, 66, 77);
        t.cube_text = Color32::from_rgb(241, 243, 245);
        t.file_rail_ink = Color32::from_rgb(241, 243, 245);
        t.logo = [Color32::from_rgb(255, 163, 110), Color32::from_rgb(255, 122, 50), Color32::from_rgb(170, 60, 10)];
        t.on_accent = Color32::from_rgb(26, 13, 4);
        t
    }

    /// Colour vision, red-green (Protan and Deutan share it, #778): values of the theme's colours
    /// that a weak red or green sense still tells apart. Each was measured against normal vision,
    /// protan and deutan, with two simulations of each loss (`colour_vision_tests`).
    pub(super) const fn red_green(self, dark: bool) -> Tokens {
        let mut t = self;
        let positive = [0.20, 0.45, 0.90];
        let negative = [0.85, 0.40, 0.05];
        t.axis_x = Color32::from_rgb(200, 40, 40);
        t.axis_y = Color32::from_rgb(120, 210, 80);
        t.preview_sketch = Color32::from_rgb(80, 185, 145);
        t.analysis.draft = [positive, [0.95, 0.90, 0.60], negative];
        t.analysis.access = [positive, negative];
        t.analysis.hatch = true;
        // Keep validation badges separate from red errors under both red-green
        // simulations, while retaining their explicit Valid / Not valid labels.
        t.form_success_fill = if dark { Color32::from_rgb(44, 57, 67) } else { Color32::from_rgb(227, 227, 246) };
        if !dark {
            t.form_success = Color32::from_rgb(39, 88, 78);
        }
        t.construction_geom = icon_primary(crate::icon_svg::CONSTRUCTION_RED_GREEN[dark as usize]);
        t.glyph_text = icon_primary(crate::icon_svg::CONSTRAINT_RED_GREEN[dark as usize]);
        // A deeper red keeps conflicting sketch geometry (#7244) apart from the orange
        // construction and centerline lines and the crimson lost links for a red-green loss.
        t.sketch_conflict = Color32::from_rgb(195, 35, 30);
        t.surface_accent = if dark { Color32::from_rgb(200, 135, 55) } else { Color32::from_rgb(164, 80, 0) };
        t.mesh_accent = if dark { Color32::from_rgb(190, 175, 70) } else { Color32::from_rgb(130, 103, 0) };
        t.form_accent = if dark { Color32::from_rgb(165, 135, 220) } else { Color32::from_rgb(115, 73, 205) };
        if dark {
            t.bend_up = Color32::from_rgb(110, 165, 255);
            t.bend_down = Color32::from_rgb(240, 180, 70);
            t.sketch_accent = icon_primary(crate::icon_svg::SKETCH_RED_GREEN[1]);
            // Graphite's error rows, brown accent tint and orange accent sit too near each other
            // for red-green eyes: a violet error tint and a yellower warning keep them apart.
            t.timeline_item_error = Color32::from_rgb(72, 60, 112);
            t.warning = Color32::from_rgb(242, 220, 96);
        } else {
            t.sketch_projected = Color32::from_rgb(205, 120, 215);
            t.sketch_locked = Color32::from_rgb(60, 150, 120);
            t.timeline_item_error = Color32::from_rgb(214, 208, 240);
            t.bend_up = Color32::from_rgb(30, 100, 210);
            t.bend_down = Color32::from_rgb(200, 120, 0);
            t.sketch_accent = icon_primary(crate::icon_svg::SKETCH_RED_GREEN[0]);
        }
        t
    }

    /// Colour vision, blue-yellow (Tritan, #778), measured against normal vision and tritan.
    pub(super) const fn blue_yellow(self, dark: bool) -> Tokens {
        let mut t = self;
        t.analysis.hatch = true;
        t.surface_accent = if dark { Color32::from_rgb(210, 120, 95) } else { Color32::from_rgb(164, 65, 50) };
        t.mesh_accent = if dark { Color32::from_rgb(200, 175, 135) } else { Color32::from_rgb(130, 103, 65) };
        t.form_accent = if dark { Color32::from_rgb(150, 140, 210) } else { Color32::from_rgb(115, 73, 170) };
        // Green and blue status lights merge for tritan vision; use rose for working.
        t.agent_busy = Color32::from_rgb(232, 150, 190);
        t.preview_sketch = Color32::from_rgb(125, 180, 105);
        t.glyph_text = icon_primary(crate::icon_svg::CONSTRAINT_BLUE_YELLOW[dark as usize]);
        t.construction_geom = icon_primary(crate::icon_svg::CONSTRUCTION_BLUE_YELLOW[dark as usize]);
        // Conflicting sketch geometry (#7244) apart from the lost links and the construction and
        // centerline lines for a blue-yellow loss: darker on the light view, lighter on the dark.
        t.sketch_conflict = if dark { Color32::from_rgb(255, 170, 160) } else { Color32::from_rgb(165, 15, 30) };
        if dark {
            t.sketch_locked = Color32::from_rgb(140, 225, 110);
            t.box_crossing = Color32::from_rgb(240, 110, 170);
        } else {
            t.sketch_locked = Color32::from_rgb(20, 165, 70);
            t.box_crossing = Color32::from_rgb(200, 60, 120);
        }
        t
    }
}

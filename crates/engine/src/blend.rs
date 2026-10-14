//! 叶子混合模式。
//!
//! 形状、文本和图片各自保存一个 [`BlendMode`]。组没有混合：选中组时补丁写到子孙叶子，
//! 绘制时每个叶子按自己的模式和绘制顺序叠到已经画好的背景上。
//!
//! ```text
//! 下层颜色 Cb、上层颜色 Cs、上层 alpha αs、下层 alpha αb
//!        │
//!        ▼
//! B(Cb, Cs) ──► Cs' = (1 - αb) * Cs + αb * B(Cb, Cs)
//!        │
//!        ▼
//! source-over（与原来的 Normal 公式相同）
//! ```
//!
//! JSON 用小写名字。缺省 [`BlendMode::Normal`]，旧文件不写这个字段也能读。

use infinitecanvas_math::Color;
use serde::{Deserialize, Serialize};

/// 混合模式。
///
/// 集合固定为这六种，和 CSS `mix-blend-mode` 同名：
/// `normal`、`multiply`、`screen`、`overlay`、`darken`、`lighten`。
///
/// `#[serde(default = Normal)]` 由 [`default_blend_mode`] 实现：缺字段时是 [`BlendMode::Normal`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BlendMode {
    /// 普通源覆盖。缺省值。
    #[default]
    Normal,
    /// 正片叠底。`Cs * Cb`。
    Multiply,
    /// 滤色。`1 - (1 - Cs) * (1 - Cb)`。
    Screen,
    /// 叠加。下层暗于 0.5 时按正片叠底，否则按滤色。
    Overlay,
    /// 变暗。逐通道取较小值。
    Darken,
    /// 变亮。逐通道取较大值。
    Lighten,
}

/// 全部混合模式，顺序与属性面板下拉一致。
pub const BLEND_MODES: [BlendMode; 6] = [
    BlendMode::Normal,
    BlendMode::Multiply,
    BlendMode::Screen,
    BlendMode::Overlay,
    BlendMode::Darken,
    BlendMode::Lighten,
];

/// serde `default = Normal`。缺字段时返回 [`BlendMode::Normal`]。
pub fn default_blend_mode() -> BlendMode {
    BlendMode::Normal
}

/// Normal 不写入 JSON，避免旧快照多出一个字段。
pub fn blend_is_normal(mode: &BlendMode) -> bool {
    *mode == BlendMode::Normal
}

impl BlendMode {
    /// 面板、JSON 和 CSS `mix-blend-mode` 用的小写名字。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Multiply => "multiply",
            Self::Screen => "screen",
            Self::Overlay => "overlay",
            Self::Darken => "darken",
            Self::Lighten => "lighten",
        }
    }

    /// 解析小写或大小写混合的名字。无法识别时返回 `None`。
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "normal" => Some(Self::Normal),
            "multiply" => Some(Self::Multiply),
            "screen" => Some(Self::Screen),
            "overlay" => Some(Self::Overlay),
            "darken" => Some(Self::Darken),
            "lighten" => Some(Self::Lighten),
            _ => None,
        }
    }
}

/// 把上层颜色按 W3C 混合后再做 source-over，得到未预乘的结果。
///
/// 上层 alpha 为 0 时原样返回下层。结果 alpha 接近 0 时返回全透明。
/// Normal 与 PNG 里原来的 source-over 是同一个式子。
///
/// # 参数
/// - `dst`: 下层颜色（未预乘）
/// - `src`: 上层颜色（未预乘）
/// - `mode`: 混合模式
pub fn composite_straight(dst: Color, src: Color, mode: BlendMode) -> Color {
    let sa = src.a.clamp(0.0, 1.0);
    if sa <= 0.0 {
        return dst;
    }
    let sr = src.r.clamp(0.0, 1.0);
    let sg = src.g.clamp(0.0, 1.0);
    let sb = src.b.clamp(0.0, 1.0);
    let da = dst.a.clamp(0.0, 1.0);
    let dr = dst.r.clamp(0.0, 1.0);
    let dg = dst.g.clamp(0.0, 1.0);
    let db = dst.b.clamp(0.0, 1.0);

    let (br, bg, bb) = blend_rgb(mode, sr, sg, sb, dr, dg, db);
    let cr = (1.0 - da) * sr + da * br;
    let cg = (1.0 - da) * sg + da * bg;
    let cb = (1.0 - da) * sb + da * bb;
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 1e-6 {
        return Color::new(0.0, 0.0, 0.0, 0.0);
    }
    let inv = 1.0 - sa;
    Color::new(
        (cr * sa + dr * da * inv) / out_a,
        (cg * sa + dg * da * inv) / out_a,
        (cb * sa + db * da * inv) / out_a,
        out_a,
    )
}

fn blend_rgb(
    mode: BlendMode,
    sr: f32,
    sg: f32,
    sb: f32,
    dr: f32,
    dg: f32,
    db: f32,
) -> (f32, f32, f32) {
    match mode {
        BlendMode::Normal => (sr, sg, sb),
        BlendMode::Multiply => (sr * dr, sg * dg, sb * db),
        BlendMode::Screen => (
            screen_channel(sr, dr),
            screen_channel(sg, dg),
            screen_channel(sb, db),
        ),
        BlendMode::Overlay => (
            overlay_channel(sr, dr),
            overlay_channel(sg, dg),
            overlay_channel(sb, db),
        ),
        BlendMode::Darken => (sr.min(dr), sg.min(dg), sb.min(db)),
        BlendMode::Lighten => (sr.max(dr), sg.max(dg), sb.max(db)),
    }
}

fn screen_channel(src: f32, dst: f32) -> f32 {
    1.0 - (1.0 - src) * (1.0 - dst)
}

fn overlay_channel(src: f32, dst: f32) -> f32 {
    if dst <= 0.5 {
        2.0 * src * dst
    } else {
        1.0 - 2.0 * (1.0 - src) * (1.0 - dst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(actual: Color, expected: Color) {
        for (got, want) in [actual.r, actual.g, actual.b, actual.a]
            .into_iter()
            .zip([expected.r, expected.g, expected.b, expected.a])
        {
            assert!((got - want).abs() < 1e-5, "{actual:?} != {expected:?}");
        }
    }

    #[test]
    fn normal_matches_source_over() {
        let dst = Color::new(0.0, 0.0, 1.0, 1.0);
        let src = Color::new(1.0, 0.0, 0.0, 0.5);
        approx(
            composite_straight(dst, src, BlendMode::Normal),
            Color::new(0.5, 0.0, 0.5, 1.0),
        );
    }

    #[test]
    fn multiply_of_blue_over_red_is_black() {
        let dst = Color::new(1.0, 0.0, 0.0, 1.0);
        let src = Color::new(0.0, 0.0, 1.0, 1.0);
        approx(
            composite_straight(dst, src, BlendMode::Multiply),
            Color::new(0.0, 0.0, 0.0, 1.0),
        );
    }

    #[test]
    fn partial_multiply_keeps_backdrop() {
        let dst = Color::new(0.0, 0.0, 1.0, 1.0);
        let src = Color::new(1.0, 0.0, 0.0, 0.5);
        approx(
            composite_straight(dst, src, BlendMode::Multiply),
            Color::new(0.0, 0.0, 0.5, 1.0),
        );
    }

    #[test]
    fn screen_darken_lighten_and_overlay() {
        let dst = Color::new(0.25, 0.75, 0.0, 1.0);
        let src = Color::new(0.25, 0.25, 1.0, 1.0);
        approx(
            composite_straight(dst, src, BlendMode::Screen),
            Color::new(
                screen_channel(0.25, 0.25),
                screen_channel(0.25, 0.75),
                screen_channel(1.0, 0.0),
                1.0,
            ),
        );
        approx(
            composite_straight(dst, src, BlendMode::Darken),
            Color::new(0.25, 0.25, 0.0, 1.0),
        );
        approx(
            composite_straight(dst, src, BlendMode::Lighten),
            Color::new(0.25, 0.75, 1.0, 1.0),
        );
        approx(
            composite_straight(dst, src, BlendMode::Overlay),
            Color::new(
                overlay_channel(0.25, 0.25),
                overlay_channel(0.25, 0.75),
                overlay_channel(1.0, 0.0),
                1.0,
            ),
        );
    }

    #[test]
    fn zero_source_alpha_leaves_destination() {
        let dst = Color::new(0.2, 0.4, 0.6, 0.8);
        let src = Color::new(1.0, 0.0, 0.0, 0.0);
        approx(composite_straight(dst, src, BlendMode::Multiply), dst);
    }

    #[test]
    fn parse_round_trip_and_default() {
        assert_eq!(default_blend_mode(), BlendMode::Normal);
        assert!(blend_is_normal(&BlendMode::Normal));
        for mode in BLEND_MODES {
            assert_eq!(BlendMode::parse(mode.as_str()), Some(mode));
            let json = serde_json::to_string(&mode).unwrap();
            let restored: BlendMode = serde_json::from_str(&json).unwrap();
            assert_eq!(restored, mode);
        }
        assert_eq!(BlendMode::parse("Multiply"), Some(BlendMode::Multiply));
        assert_eq!(BlendMode::parse("nope"), None);
    }
}

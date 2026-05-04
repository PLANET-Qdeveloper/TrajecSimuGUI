use plotters::prelude::*;
use std::error::Error;
use std::path::PathBuf;

use simulator_core::{EventKind, EventStamp, SimulationState, UnifiedSimulationOutput};

// 岡部・伊藤のカラーパレット指定 (Vermilion -> Orange -> Blue)
const COLOR_VERMILION: RGBColor = RGBColor(213, 94, 0);
const COLOR_ORANGE: RGBColor = RGBColor(230, 159, 0);
const COLOR_BLUE: RGBColor = RGBColor(0, 114, 178);
const PALETTE: [RGBColor; 3] = [COLOR_VERMILION, COLOR_ORANGE, COLOR_BLUE];

/// グラフに描画するデータシリーズ
pub struct SeriesData {
    pub label: String,
    pub data: Vec<(f64, f64)>,
}

/// 特定の点に追加するアノテーション（文字付きポイント）
pub struct Annotation {
    pub x: f64,
    pub y: f64,
    pub text: String,
}

/// グラフの全体設定
pub struct PlotConfig {
    pub output_path: PathBuf,
    pub x_label: String, // 単位を含むX軸ラベル (例: "Time [s]")
    pub y_label: String, // 単位を含むY軸ラベル (例: "Voltage [V]")
    pub x_range: Option<(f64, f64)>,
    pub y_range: Option<(f64, f64)>,
    pub is_x_log: bool,  // X軸を対数にするか
    pub is_y_log: bool,  // Y軸を対数にするか
    pub annotations: Vec<Annotation>,
}

fn validate_and_bounds(
    series_list: &[SeriesData],
    is_x_log: bool,
    is_y_log: bool,
) -> Result<((f64, f64), (f64, f64)), Box<dyn Error>> {
    let mut x_min = f64::INFINITY;
    let mut x_max = f64::NEG_INFINITY;
    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;

    for series in series_list {
        for &(x, y) in &series.data {
            if x < x_min {
                x_min = x;
            }
            if x > x_max {
                x_max = x;
            }
            if y < y_min {
                y_min = y;
            }
            if y > y_max {
                y_max = y;
            }
        }
    }

    if !x_min.is_finite() || !y_min.is_finite() {
        return Err("series_list has no data points".into());
    }
    if x_min == x_max || y_min == y_max {
        return Err("data points collapse to a single point".into());
    }
    if is_x_log && x_min <= 0.0 {
        return Err("x data contains non-positive values for log axis".into());
    }
    if is_y_log && y_min <= 0.0 {
        return Err("y data contains non-positive values for log axis".into());
    }

    Ok(((x_min, x_max), (y_min, y_max)))
}

pub fn draw_result_plot(path: &std::path::Path,
                        output: &UnifiedSimulationOutput,) -> Result<(), Box<dyn Error>> {
    let mut qbar_plot_config = PlotConfig{
        output_path: PathBuf::from(path.join("qbar.plot.bmp")),
        x_label: "Time (s)".to_string(),
        y_label: "Dynamic Pressure (Pa)".to_string(),
        x_range: None,
        y_range: None,
        is_x_log: false,
        is_y_log: false,
        annotations: vec![],
    };
    Ok(())

}

/// 学術的なグラフを生成する関数
pub fn draw_academic_plot(config: PlotConfig, series_list: Vec<SeriesData>) -> Result<(), Box<dyn Error>> {
    let root = BitMapBackend::new(&config.output_path, (800, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let ((data_x_min, data_x_max), (data_y_min, data_y_max)) =
        validate_and_bounds(&series_list, config.is_x_log, config.is_y_log)?;

    let (x_min, x_max) = config.x_range.unwrap_or((data_x_min, data_x_max));
    let (y_min, y_max) = if let Some(range) = config.y_range {
        range
    } else {
        let pad = (data_y_max - data_y_min).abs() * 0.05;
        (data_y_min - pad, data_y_max + pad)
    };

    if config.is_x_log && x_min <= 0.0 {
        return Err("x_range contains non-positive values for log axis".into());
    }
    if config.is_y_log && y_min <= 0.0 {
        return Err("y_range contains non-positive values for log axis".into());
    }

    // Plottersは座標系(Linear/Log)によって型が変わるため、マクロで描画処理を共通化
    macro_rules! build_and_draw {
        ($x_spec:expr, $y_spec:expr) => {{
            let mut chart = ChartBuilder::on(&root)
                .margin(20)
                // タイトルはなし
                .x_label_area_size(50)
                .y_label_area_size(60)
                .build_cartesian_2d($x_spec, $y_spec)?;

            // 軸・目盛り・グリッド線の設定 (詳細目盛りを含む)
            chart
                .configure_mesh()
                .x_desc(&config.x_label)
                .y_desc(&config.y_label)
                .axis_desc_style(("sans-serif", 20, &BLACK))
                .label_style(("sans-serif", 15, &BLACK))
                .bold_line_style(&BLACK.mix(0.2))       // 主グリッド
                .light_line_style(&BLACK.mix(0.05))      // 詳細(マイナー)グリッド
                .x_labels(10)
                .y_labels(10)
                .draw()?;

            // データのプロット (1〜3個のシリーズを想定)
            for (i, series) in series_list.iter().enumerate() {
                // 色を順番に適用 (3つを超過した場合はループ)
                let color = PALETTE[i % PALETTE.len()];

                chart
                    .draw_series(LineSeries::new(
                        series.data.clone(),
                        color.stroke_width(2),
                    ))?
                    .label(&series.label)
                    .legend(move |(x, y)| {
                        PathElement::new(vec![(x, y), (x + 20, y)], color.stroke_width(2))
                    });
            }

            // アノテーション(特定の点への文字付きポイント)の描画
            for anno in &config.annotations {
                chart.draw_series(std::iter::once(
                    EmptyElement::at((anno.x, anno.y))
                        + Circle::new((0, 0), 4, ShapeStyle::from(&BLACK).filled())
                        + Text::new(
                            anno.text.clone(),
                            (8, -12), // ポイントからの相対位置オフセット
                            ("sans-serif", 16).into_font(),
                        ),
                ))?;
            }

            // 凡例の描画設定
            chart
                .configure_series_labels()
                .background_style(&WHITE.mix(0.8))
                .border_style(&BLACK)
                .position(SeriesLabelPosition::UpperRight)
                .label_font(("sans-serif", 16))
                .draw()?;
        }};
    }

    // 対数軸の設定フラグに応じて構築を分岐
    match (config.is_x_log, config.is_y_log) {
        (false, false) => build_and_draw!(x_min..x_max, y_min..y_max),
        (true, false) => build_and_draw!((x_min..x_max).log_scale(), y_min..y_max),
        (false, true) => build_and_draw!(x_min..x_max, (y_min..y_max).log_scale()),
        (true, true) => build_and_draw!((x_min..x_max).log_scale(), (y_min..y_max).log_scale()),
    }

    root.present()?;
    Ok(())
}

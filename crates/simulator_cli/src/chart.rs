use plotters::prelude::*;
use std::error::Error;
use std::path::PathBuf;

use simulator_core::UnifiedSimulationOutput;

// 岡部・伊藤のカラーパレット指定 (Vermilion -> Orange -> Blue)
const COLOR_VERMILION: RGBColor = RGBColor(213, 94, 0);
const REDDISH_PURPLE: RGBColor = RGBColor(204, 121, 167);
const COLOR_BLUE: RGBColor = RGBColor(0, 114, 178);
const PALETTE: [RGBColor; 3] = [COLOR_VERMILION, REDDISH_PURPLE, COLOR_BLUE];

/// グラフに描画するデータシリーズ
pub struct SeriesData<'a> {
    pub x_axis: &'a Vec<f64>,
    pub y_axis: Vec<(Option<String>, &'a Vec<f64>)>,
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
    pub is_x_log: bool, // X軸を対数にするか
    pub is_y_log: bool, // Y軸を対数にするか
    pub annotations: Vec<Annotation>,
}

fn validate_and_bounds(
    data: &SeriesData,
    is_x_log: bool,
    is_y_log: bool,
) -> Result<((f64, f64), (f64, f64)), Box<dyn Error>> {
    let mut x_min = f64::INFINITY;
    let mut x_max = f64::NEG_INFINITY;
    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;

    for x in data.x_axis.iter() {
        if *x < x_min {
            x_min = *x;
        }
        if *x > x_max {
            x_max = *x;
        }
    }

    for y_axis in data.y_axis.iter() {
        for y in y_axis.1.iter() {
            if *y < y_min {
                y_min = *y;
            }
            if *y > y_max {
                y_max = *y;
            }
        }
    }

    if !x_min.is_finite() || !y_min.is_finite() {
        return Err("data has no data points".into());
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

pub fn draw_result_plot(
    path: &std::path::Path,
    output: &UnifiedSimulationOutput,
) -> Result<(), Box<dyn Error>> {
    let qbar_plot_config = PlotConfig {
        output_path: path.join("qbar.plot.bmp"),
        x_label: "Time (s)".to_string(),
        y_label: "Dynamic Pressure (Pa)".to_string(),
        x_range: None,
        y_range: None,
        is_x_log: false,
        is_y_log: false,
        annotations: vec![],
    };

    draw_academic_plot(
        qbar_plot_config,
        SeriesData {
            x_axis: &output.mainline.trajectory.time_sec,
            y_axis: vec![(None, &output.mainline.trajectory.qbar_pa)],
        },
    )
    .expect("failed to draw qbar plot");

    let mach_plot_config = PlotConfig {
        output_path: path.join("mach.plot.bmp"),
        x_label: "Time (s)".to_string(),
        y_label: "Mach".to_string(),
        x_range: None,
        y_range: None,
        is_x_log: false,
        is_y_log: false,
        annotations: vec![],
    };

    draw_academic_plot(
        mach_plot_config,
        SeriesData {
            x_axis: &output.mainline.trajectory.time_sec,
            y_axis: vec![(None, &output.mainline.trajectory.mach)],
        },
    )
    .expect("failed to draw mach plot");

    let altitude_plot_config = PlotConfig {
        output_path: path.join("altitude.plot.bmp"),
        x_label: "Time (s)".to_string(),
        y_label: "Altitude (m)".to_string(),
        x_range: None,
        y_range: None,
        is_x_log: false,
        is_y_log: false,
        annotations: vec![],
    };

    draw_academic_plot(
        altitude_plot_config,
        SeriesData {
            x_axis: &output.mainline.trajectory.time_sec,
            y_axis: vec![(None, &output.mainline.trajectory.alt_msl_m)],
        },
    )
    .expect("failed to draw mach plot");

    let velocity_plot_config = PlotConfig {
        output_path: path.join("velocity.plot.bmp"),
        x_label: "Time (s)".to_string(),
        y_label: "Velocity (m/s)".to_string(),
        x_range: None,
        y_range: None,
        is_x_log: false,
        is_y_log: false,
        annotations: vec![],
    };

    draw_academic_plot(
        velocity_plot_config,
        SeriesData {
            x_axis: &output.mainline.trajectory.time_sec,
            y_axis: vec![
                (Some("x".to_string()), &output.mainline.trajectory.u_mps),
                (Some("y".to_string()), &output.mainline.trajectory.v_mps),
                (Some("z".to_string()), &output.mainline.trajectory.w_mps),
            ],
        },
    )
    .expect("failed to draw mach plot");

    let trajectory_plot_config = PlotConfig {
        output_path: path.join("trajectory.plot.bmp"),
        x_label: "Time (s)".to_string(),
        y_label: "xyz (m)".to_string(),
        x_range: None,
        y_range: None,
        is_x_log: false,
        is_y_log: false,
        annotations: vec![],
    };
    draw_academic_plot(
        trajectory_plot_config,
        SeriesData {
            x_axis: &output.mainline.trajectory.time_sec,
            y_axis: vec![
                (Some("x".to_string()), &output.mainline.trajectory.local_x_m),
                (Some("y".to_string()), &output.mainline.trajectory.local_y_m),
                (
                    Some("altitude".to_string()),
                    &output.mainline.trajectory.alt_msl_m,
                ),
            ],
        },
    )
    .expect("failed to draw mach plot");

    let acceleration_plot_config = PlotConfig {
        output_path: path.join("acceleration.plot.bmp"),
        x_label: "Time (s)".to_string(),
        y_label: "xyz (m/s²)".to_string(),
        x_range: None,
        y_range: None,
        is_x_log: false,
        is_y_log: false,
        annotations: vec![],
    };

    draw_academic_plot(
        acceleration_plot_config,
        SeriesData {
            x_axis: &output.mainline.trajectory.time_sec,
            y_axis: vec![
                (Some("x".to_string()), &output.mainline.trajectory.ax_mps2),
                (Some("y".to_string()), &output.mainline.trajectory.ay_mps2),
                (Some("z".to_string()), &output.mainline.trajectory.az_mps2),
            ],
        },
    )
    .expect("failed to draw mach plot");

    Ok(())
}

/// 学術的なグラフを生成する関数
pub fn draw_academic_plot(config: PlotConfig, data: SeriesData) -> Result<(), Box<dyn Error>> {
    let root = BitMapBackend::new(&config.output_path, (2000, 1500)).into_drawing_area();
    root.fill(&WHITE)?;

    let ((data_x_min, data_x_max), (data_y_min, data_y_max)) =
        validate_and_bounds(&data, config.is_x_log, config.is_y_log)?;

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

    // 凡例に表示するラベルが1つでも存在するかチェック
    let has_legend = data.y_axis.iter().any(|(label, _)| label.is_some());

    // Plottersは座標系(Linear/Log)によって型が変わるため、マクロで描画処理を共通化
    macro_rules! build_and_draw {
        ($x_spec:expr, $y_spec:expr) => {{
            let mut chart = ChartBuilder::on(&root)
                .margin(40) // 余白も少し広めにとる
                // 軸の説明(y_desc等)と目盛りの数字が重ならないようエリアを大幅に広げる
                .x_label_area_size(100)
                .y_label_area_size(150)
                .build_cartesian_2d($x_spec, $y_spec)?;

            // 軸・目盛り・グリッド線の設定 (詳細目盛りを含む)
            chart
                .configure_mesh()
                .x_desc(&config.x_label)
                .y_desc(&config.y_label)
                // 2000x1500の画像サイズに合わせてフォントサイズを大幅に拡大 (20 -> 50)
                .axis_desc_style(("sans-serif", 50, &BLACK))
                .label_style(("sans-serif", 40, &BLACK))
                // 縦軸の数字の桁数を制限して重なりを防ぐ (必要に応じて {:.2e} など指数表記も有効)
                .y_label_formatter(&|y| {
                    if y.abs() >= 1e4 {
                        format!("{:.1e}", y) // 指数表記 (例: 1.0e4)
                    } else {
                        format!("{:.1}", y)  // 通常表記 (例: 9999.9)
                    }
                })
                .bold_line_style(&BLACK.mix(0.2))       // 主グリッド
                .light_line_style(&BLACK.mix(0.05))      // 詳細(マイナー)グリッド
                .x_labels(10)
                .y_labels(10)
                .draw()?;

            // データのプロット (1〜3個のシリーズを想定)
            for (i, (label, y_values)) in data.y_axis.iter().enumerate() {
                let color = PALETTE[i % PALETTE.len()];

                let series = LineSeries::new(
                    data.x_axis.iter().zip(y_values.iter()).map(|(&x, &y)| (x, y)),
                    color.stroke_width(3), // 論文用に線を少し太く(2 -> 3)すると視認性が上がります
                );

                if let Some(label_text) = label {
                    chart
                        .draw_series(series)?
                        .label(label_text)
                        .legend(move |(x, y)| {
                            PathElement::new(vec![(x, y), (x + 30, y)], color.stroke_width(3))
                        });
                } else {
                    chart.draw_series(series)?;
                }
            }

            // アノテーション(特定の点への文字付きポイント)の描画
            for anno in &config.annotations {
                chart.draw_series(std::iter::once(
                    EmptyElement::at((anno.x, anno.y))
                        + Circle::new((0, 0), 6, ShapeStyle::from(&BLACK).filled()) // 点も少し大きく
                        + Text::new(
                            anno.text.clone(),
                            (12, -18), // フォント拡大に合わせてオフセットも広げる
                            ("sans-serif", 35).into_font(), // 16 -> 35 に拡大
                        ),
                ))?;
            }

            // ラベルが1つ以上ある場合のみ凡例を描画する
            if has_legend {
                chart
                    .configure_series_labels()
                    .background_style(&WHITE.mix(0.9)) // 少し透明度を下げる
                    .border_style(&BLACK)
                    .position(SeriesLabelPosition::UpperRight)
                    .label_font(("sans-serif", 35)) // 16 -> 35 に拡大
                    .margin(15) // 凡例内の余白
                    .draw()?;
            }
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

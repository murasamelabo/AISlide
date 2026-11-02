use crate::{model::chart_format::{self, AxisLabelPosition, ChartAxis, ChartOptions, LabelPosition, LegendPosition, PresentationPoint}, design::Theme, model::{ChartKind, ChartSeries}, Error, Result};
use xmlwriter::{Options, XmlWriter};

pub(super) fn required(series: &[ChartSeries], options: &ChartOptions) -> bool {
    series.iter().any(|entry| entry.trendline.is_some() || entry.error_bars.is_some())
        || [&options.primary_axis, &options.secondary_axis].iter().any(|axis| axis.major_unit.is_some() || axis.minor_unit.is_some() || axis.log_base.is_some() || axis.reverse || axis.number_format.is_some() || axis.label_position.is_some())
        || !options.category_axis.is_default()
        || options.data_labels.as_ref().is_some_and(|labels| labels.number_format.is_some())
}

pub(super) fn legend_layout(series: &[ChartSeries], left: f64, width: f64, theme: &Theme, fonts: &mut cosmic_text::FontSystem) -> (usize, Vec<(f64, usize)>) {
    if series.is_empty() { return (0, Vec::new()); }
    let available = (width - left - 12.0).max(60.0);
    let (mut row, mut cursor, mut items) = (0usize, 0.0f64, Vec::with_capacity(series.len()));
    for source in series {
        let item_width = 14.0 + super::chart_text_extent(fonts, &source.name, 11.0, crate::fonts::requested_family(&source.name, None, theme));
        if cursor > 0.0 && cursor + item_width > available { row += 1; cursor = 0.0; }
        items.push((left + cursor, row));
        cursor += item_width + 14.0;
    }
    (row + 1, items)
}

fn line(writer: &mut XmlWriter, from: [f64; 2], to: [f64; 2], color: &str, width: f64) {
    writer.start_element("line");
    for (name, value) in [("x1", from[0]), ("y1", from[1]), ("x2", to[0]), ("y2", to[1]), ("stroke-width", width)] { writer.write_attribute(name, &value); }
    writer.write_attribute("stroke", color);
    writer.end_element();
}

fn text(writer: &mut XmlWriter, position: [f64; 2], value: &str, color: &str, size: f64, anchor: &str) {
    writer.start_element("text");
    writer.write_attribute("x", &position[0]); writer.write_attribute("y", &position[1]);
    writer.write_attribute("fill", color); writer.write_attribute("font-size", &size);
    writer.write_attribute("text-anchor", anchor);
    writer.write_text(&super::escape_text(value));
    writer.end_element();
}

pub(super) fn svg(kind: ChartKind, categories: &[String], series: &[ChartSeries], options: &ChartOptions, width: f64, height: f64, theme: &Theme, fonts: &mut cosmic_text::FontSystem) -> Result<String> {
    if width < 160.0 || height < 120.0 { return Err(Error::Unsupported("statistical chart needs at least 160 x 120".into())); }
    let model = chart_format::compute_chart_presentation(kind, categories, series, options)?;
    let color = |value: &str| format!("#{}", value.strip_prefix('@').and_then(|slot| theme.colors.get(slot)).map_or(value, String::as_str));
    let ink = color("@dk1");
    let grid = color("@lt2");
    let mut writer = XmlWriter::new(Options { indent: xmlwriter::Indent::None, ..Default::default() });
    writer.start_element("svg");
    writer.write_attribute("xmlns", "http://www.w3.org/2000/svg");
    writer.write_attribute("width", &width); writer.write_attribute("height", &height);
    let horizontal = kind.is_horizontal();
    let primary_hidden = options.primary_axis.label_position == Some(AxisLabelPosition::Hidden);
    let secondary_hidden = options.secondary_axis.label_position == Some(AxisLabelPosition::Hidden);
    let category_hidden = options.category_axis.label_position == Some(AxisLabelPosition::Hidden);
    let left_hidden = if horizontal { category_hidden } else { primary_hidden };
    let bottom_hidden = if horizontal { primary_hidden } else { category_hidden };
    let equations: Vec<_> = model.series.iter().zip(series).flat_map(|(entry, source)| {
        let mut descriptions = Vec::new();
        if let Some(trend) = &entry.trend {
            if let Some(equation) = &trend.equation { descriptions.push(equation.clone()); }
            if source.trendline.as_ref().is_some_and(|config| config.display_r_squared) { descriptions.push(trend.r_squared.map_or("R squared undefined".into(), |value| format!("R squared = {value:.6}"))); }
        }
        descriptions
    }).collect();
    let top = 10.0 + equations.len() as f64 * 12.0;
    let legend = options.legend.unwrap_or(LegendPosition::Bottom);
    let widest = |fonts: &mut cosmic_text::FontSystem, axis: &chart_format::PresentationAxis| axis.ticks.iter()
        .map(|tick| super::chart_text_extent(fonts, &tick.label, 10.0, crate::fonts::requested_family(&tick.label, None, theme))).fold(0.0, f64::max);
    let left = if left_hidden { 16.0 } else { (widest(fonts, if horizontal { &model.category_axis } else { &model.primary_axis }) + 12.0).max(if horizontal { 80.0 } else { 58.0 }) };
    let right = if horizontal { if primary_hidden { 20.0 } else { 58.0 } } else { model.secondary_axis.as_ref().map_or(58.0, |axis| if secondary_hidden { 20.0 } else { (widest(fonts, axis) + 12.0).max(58.0) }) };
    let plot_width = width - left - right;
    if plot_width < 30.0 { return Err(Error::Unsupported("chart axis labels do not fit the requested font and format; enlarge the chart or use a compact number format".into())); }
    let (legend_rows, legend_items) = if legend == LegendPosition::Hidden { (0, Vec::new()) } else { legend_layout(series, left, width, theme, fonts) };
    let plot_height = height - top - if bottom_hidden { 10.0 } else { 28.0 } - if legend_rows == 0 { 0.0 } else { legend_rows as f64 * 15.0 + 6.0 };
    if plot_height < 30.0 { return Err(Error::Unsupported("chart equations/legend do not fit; enlarge chart".into())); }
    for (index, equation) in equations.iter().enumerate() { text(&mut writer, [left, index as f64 * 12.0], equation, &ink, (plot_width / (equation.len().max(1) as f64 * 0.6)).clamp(4.0, 10.0), "start"); }
    let project = |point: &PresentationPoint, axis: &chart_format::PresentationAxis| -> Result<[f64; 2]> {
        let category = model.category_axis.project(point.x)?;
        let value = axis.project(point.y)?;
        Ok(if horizontal { [value * plot_width, category * plot_height] } else { [category * plot_width, (1.0 - value) * plot_height] })
    };
    let primary = &model.primary_axis;
    for (secondary, axis) in [(false, Some(primary)), (true, model.secondary_axis.as_ref())] {
        let Some(axis) = axis else { continue; };
        let axis_hidden = if secondary { secondary_hidden } else { primary_hidden };
        for tick in axis.minor_ticks.iter().chain(&axis.ticks) {
            if horizontal { line(&mut writer, [left + tick.position * plot_width, top], [left + tick.position * plot_width, top + plot_height], &grid, 0.5); }
            else { line(&mut writer, [left, top + (1.0 - tick.position) * plot_height], [left + plot_width, top + (1.0 - tick.position) * plot_height], &grid, 0.5); }
        }
        if !axis_hidden {
            for tick in &axis.ticks {
                let position = if horizontal { [left + tick.position * plot_width, if secondary { top - 12.0 } else { top + plot_height + 4.0 }] } else { [if secondary { left + plot_width + 4.0 } else { left - 4.0 }, top + (1.0 - tick.position) * plot_height - 6.0] };
                text(&mut writer, position, &tick.label, &ink, 10.0, if horizontal { "middle" } else if secondary { "start" } else { "end" });
            }
        }
    }
    if !category_hidden {
        for tick in &model.category_axis.ticks {
            let position = if horizontal { [left - 5.0, top + tick.position * plot_height - 6.0] } else { [left + tick.position * plot_width, top + plot_height + 4.0] };
            text(&mut writer, position, &tick.label, &ink, 10.0, if horizontal { "end" } else { "middle" });
        }
    }
    writer.start_element("svg");
    writer.write_attribute("x", &left); writer.write_attribute("y", &top);
    writer.write_attribute("width", &plot_width); writer.write_attribute("height", &plot_height);
    writer.write_attribute("viewBox", &format!("0 0 {plot_width} {plot_height}"));
    writer.write_attribute("overflow", "hidden");
    let mut positive = vec![0.0; categories.len()];
    let mut negative = positive.clone();
    let stacked = kind.is_percent() || matches!(kind, ChartKind::StackedBar | ChartKind::StackedColumn);
    let requested_label_pos = options.data_labels.as_ref().and_then(|labels| labels.position);
    let mut deferred_labels: Vec<([f64; 2], String, &'static str)> = Vec::new();
    for (index, entry) in model.series.iter().enumerate() {
        let source = &series[index];
        let axis = if entry.axis == ChartAxis::Secondary { model.secondary_axis.as_ref().unwrap_or(primary) } else { primary };
        let shade = color(&source.color);
        let actual = source.kind.unwrap_or(kind);
        let baseline = if axis.log_base.is_some() { axis.domain[0] } else { 0.0f64.clamp(axis.domain[0], axis.domain[1]) };
        let mut path = String::new();
        for (position, point) in entry.points.iter().enumerate() {
            let coordinate = project(point, axis)?;
            path.push_str(&format!("{}{} {} ", if position == 0 { "M" } else { "L" }, coordinate[0], coordinate[1]));
            if actual.is_bar() {
                let start = if stacked { if point.y >= 0.0 { positive[position] } else { negative[position] } } else { baseline };
                let end = if stacked { start + point.y } else { point.y };
                if point.y >= 0.0 { positive[position] = end; } else { negative[position] = end; }
                let mut first = project(&PresentationPoint { x: point.x, y: start }, axis)?;
                let mut last = project(&PresentationPoint { x: point.x, y: end }, axis)?;
                let thickness = (if horizontal { plot_height } else { plot_width }) / (model.category_axis.domain[1] - model.category_axis.domain[0]) * 0.7 / if stacked { 1.0 } else { series.len() as f64 };
                let shift = if stacked { -thickness / 2.0 } else { (index as f64 - series.len() as f64 / 2.0) * thickness };
                let cross = usize::from(horizontal);
                first[cross] += shift; last[cross] = first[cross] + thickness;
                let bar_w = (last[0] - first[0]).abs();
                let bar_h = (last[1] - first[1]).abs();
                if !stacked || point.y != 0.0 {
                    writer.start_element("rect");
                    for (name, value) in [("x", first[0].min(last[0])), ("y", first[1].min(last[1])), ("width", bar_w), ("height", bar_h)] { writer.write_attribute(name, &value); }
                    writer.write_attribute("fill", &shade); writer.end_element();
                }
                if !entry.labels[position].is_empty() && (!stacked || point.y != 0.0) {
                    let effective_pos = requested_label_pos.unwrap_or(if stacked { LabelPosition::Center } else { LabelPosition::OutsideEnd });
                    let (label_coord, anchor) = if horizontal {
                        let mid_y = (first[1] + last[1]) / 2.0 - 5.0;
                        match effective_pos {
                            LabelPosition::Center | LabelPosition::BestFit => ([(first[0] + last[0]) / 2.0, mid_y], "middle"),
                            LabelPosition::InsideEnd if bar_w > 18.0 => if last[0] >= first[0] { ([last[0] - 4.0, mid_y], "end") } else { ([last[0] + 4.0, mid_y], "start") },
                            LabelPosition::InsideEnd => ([(first[0] + last[0]) / 2.0, mid_y], "middle"),
                            LabelPosition::OutsideEnd => if last[0] >= first[0] { ([(last[0] + 4.0).min(plot_width - 2.0), mid_y], "start") } else { ([(last[0] - 4.0).max(2.0), mid_y], "end") },
                        }
                    } else {
                        let (mid_x, positive_dir) = ((first[0] + last[0]) / 2.0, last[1] <= first[1]);
                        match effective_pos {
                            LabelPosition::Center | LabelPosition::BestFit => ([mid_x, (first[1] + last[1]) / 2.0 - 5.0], "middle"),
                            LabelPosition::InsideEnd if bar_h > 16.0 => ([mid_x, if positive_dir { last[1] + 2.0 } else { last[1] - 12.0 }], "middle"),
                            LabelPosition::InsideEnd => ([mid_x, (first[1] + last[1]) / 2.0 - 5.0], "middle"),
                            LabelPosition::OutsideEnd => ([mid_x, if positive_dir { (last[1] - 12.0).max(0.0) } else { (last[1] + 2.0).min(plot_height - 10.0) }], "middle"),
                        }
                    };
                    deferred_labels.push((label_coord, entry.labels[position].clone(), anchor));
                }
            } else {
                writer.start_element("circle"); writer.write_attribute("cx", &coordinate[0]); writer.write_attribute("cy", &coordinate[1]);
                let radius = if actual == ChartKind::Bubble { let sizes = source.bubble_sizes.as_ref().ok_or_else(|| Error::Invalid("bubble sizes missing".into()))?; (sizes[position] / sizes.iter().copied().fold(1.0, f64::max)).sqrt() * 16.0 } else { 3.0 };
                writer.write_attribute("r", &radius); writer.write_attribute("fill", &shade); writer.end_element();
                if !entry.labels[position].is_empty() {
                    deferred_labels.push(([coordinate[0], coordinate[1] - 14.0], entry.labels[position].clone(), "middle"));
                }
            }
        }
        if matches!(actual, ChartKind::Line | ChartKind::Area) {
            if actual == ChartKind::Area { let first = project(&PresentationPoint { x: entry.points[0].x, y: baseline }, axis)?; let last = project(&PresentationPoint { x: entry.points.last().unwrap().x, y: baseline }, axis)?; path.push_str(&format!("L{} {} L{} {} Z", last[0], last[1], first[0], first[1])); }
            writer.start_element("path"); writer.write_attribute("d", &path); writer.write_attribute("fill", if actual == ChartKind::Area { shade.as_str() } else { "none" }); writer.write_attribute("fill-opacity", &0.35); writer.write_attribute("stroke", &shade); writer.write_attribute("stroke-width", &2); writer.end_element();
        }
        for error in &entry.errors {
            let lower = project(&error.lower, axis)?; let upper = project(&error.upper, axis)?;
            line(&mut writer, lower, upper, &shade, 1.5);
            let vertical = (error.direction == chart_format::ErrorBarDirection::Y) != horizontal;
            for endpoint in [lower, upper] { line(&mut writer, [endpoint[0] - if vertical { 4.0 } else { 0.0 }, endpoint[1] - if vertical { 0.0 } else { 4.0 }], [endpoint[0] + if vertical { 4.0 } else { 0.0 }, endpoint[1] + if vertical { 0.0 } else { 4.0 }], &shade, 1.5); }
        }
        if let Some(trend) = &entry.trend {
            let mut path = String::new();
            for (position, point) in trend.points.iter().enumerate() { let coordinate = project(point, axis)?; path.push_str(&format!("{}{} {} ", if position == 0 { "M" } else { "L" }, coordinate[0], coordinate[1])); }
            writer.start_element("path"); writer.write_attribute("d", &path); writer.write_attribute("fill", "none"); writer.write_attribute("stroke", &shade); writer.write_attribute("stroke-width", &2); writer.write_attribute("stroke-dasharray", "6 4"); writer.end_element();
        }
    }
    for (coord, label_text, anchor) in deferred_labels {
        text(&mut writer, coord, &label_text, &ink, 10.0, anchor);
    }
    writer.end_element();
    if legend != LegendPosition::Hidden {
        let legend_top = height - legend_rows as f64 * 15.0;
        for ((item_x, row), source) in legend_items.into_iter().zip(series) {
            let row_y = legend_top + row as f64 * 15.0;
            let shade = color(&source.color);
            writer.start_element("rect");
            for (name, value) in [("x", item_x), ("y", row_y + 2.0), ("width", 10.0), ("height", 10.0)] {
                writer.write_attribute(name, &value);
            }
            writer.write_attribute("fill", &shade);
            writer.end_element();
            text(&mut writer, [item_x + 14.0, row_y], &source.name, &ink, 11.0, "start");
        }
    }
    writer.end_element();
    Ok(writer.end_document())
}
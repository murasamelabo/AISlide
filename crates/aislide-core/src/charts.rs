use crate::{model::{ChartKind, ChartSeries, ChartOptions, ChartAxis, chart_format::*}, package::Package, pptx::{color, empty, xml}, Result};
use std::collections::BTreeMap;
use xmlwriter::XmlWriter;

pub(crate) const CHART_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const SHEET_NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";

fn cache(node: roxmltree::Node<'_, '_>) -> Result<Vec<String>> {
    let mut points = BTreeMap::new();
    for point in node.descendants().filter(|node| node.has_tag_name((CHART_NS, "pt"))) {
        let index: usize = point.attribute("idx").and_then(|value| value.parse().ok()).ok_or_else(|| crate::Error::Unsupported("chart cache index".into()))?;
        if index >= 32 || points.insert(index, crate::native::child(point, CHART_NS, "v").and_then(|node| node.text()).unwrap_or("").to_owned()).is_some() { return Err(crate::Error::Unsupported("sparse or oversized chart cache".into())); }
    }
    if points.keys().copied().ne(0..points.len()) { return Err(crate::Error::Unsupported("sparse chart cache".into())); }
    if node.descendants().filter(|node| node.has_tag_name((CHART_NS, "ptCount"))).any(|node| node.attribute("val").and_then(|value| value.parse::<usize>().ok()) != Some(points.len())) { return Err(crate::Error::Unsupported("chart cache count mismatch".into())); }
    Ok(points.into_values().collect())
}

pub(crate) fn numeric_cache(node: roxmltree::Node<'_, '_>) -> Result<Vec<f64>> {
    cache(node)?.into_iter().map(|value| value.parse::<f64>().map_err(|_| crate::Error::Unsupported("nonnumeric chart cache".into()))).collect()
}

fn read_kind(node: roxmltree::Node<'_, '_>) -> Result<ChartKind> {
    use crate::native::child;
    let value = |tag: &str| child(node, CHART_NS, tag).and_then(|node| node.attribute("val"));
    Ok(match node.tag_name().name() {
        "barChart" => match (value("barDir"), value("grouping")) {
            (Some("col"), Some("percentStacked")) => ChartKind::PercentStackedColumn,
            (Some("bar"), Some("percentStacked")) => ChartKind::PercentStackedBar,
            (Some("col"), Some("stacked")) => ChartKind::StackedColumn,
            (Some("bar"), Some("stacked")) => ChartKind::StackedBar,
            (Some("col"), None | Some("clustered")) => ChartKind::Column,
            (Some("bar"), None | Some("clustered")) => ChartKind::Bar,
            _ => return Err(crate::Error::Unsupported("chart bar grouping/direction".into())),
        },
        "bar3DChart" if value("grouping") == Some("clustered") => match value("barDir") { Some("bar") => ChartKind::Bar3d, Some("col") => ChartKind::Column3d, _ => return Err(crate::Error::Unsupported("3D bar direction".into())) },
        "lineChart" if matches!(value("grouping"), None | Some("standard")) => ChartKind::Line,
        "areaChart" if matches!(value("grouping"), None | Some("standard")) => ChartKind::Area,
        "pieChart" => ChartKind::Pie, "doughnutChart" => ChartKind::Doughnut, "pie3DChart" => ChartKind::Pie3d,
        "scatterChart" => ChartKind::Scatter, "bubbleChart" => ChartKind::Bubble,
        "radarChart" => match value("radarStyle") { Some("filled") => ChartKind::RadarFilled, Some("marker" | "standard") => ChartKind::Radar, _ => return Err(crate::Error::Unsupported("radar style".into())) },
        _ => return Err(crate::Error::Unsupported("chart family/grouping".into())),
    })
}

pub(crate) fn read(document: &roxmltree::Document<'_>, read_color: fn(roxmltree::Node<'_, '_>, &str) -> String) -> Result<(ChartKind, Vec<String>, Vec<ChartSeries>, ChartOptions)> {
    if document.root_element().has_tag_name((crate::chart_extended::NS, "chartSpace")) { return crate::chart_extended::read(document, read_color); }
    use crate::native::{A, C, child};
    use crate::Error;
    let chart = child(document.root_element(), C, "chart").ok_or_else(|| Error::Unsupported("chart missing".into()))?;
    let plot = child(chart, C, "plotArea").ok_or_else(|| Error::Unsupported("chart plot missing".into()))?;
    let groups: Vec<_> = plot.children().filter(|node| node.tag_name().namespace() == Some(C) && node.tag_name().name().ends_with("Chart")).collect();
    if groups.is_empty() || groups.len() > 6 { return Err(Error::Unsupported("chart group count".into())); }
    let kind = if groups.len() > 1 { ChartKind::Combo } else { read_kind(groups[0])? };
    let axes: BTreeMap<_, _> = plot.children().filter(|node| node.tag_name().namespace() == Some(C) && ["catAx", "valAx"].contains(&node.tag_name().name())).map(|node| Ok((child(node, C, "axId").and_then(|node| node.attribute("val")).ok_or_else(|| Error::Unsupported("axis ID missing".into()))?, node))).collect::<Result<_>>()?;
    let mut axis_pairs: Vec<(ChartAxis, &str, &str)> = Vec::new();
    let mut categories = Vec::new(); let mut ordered = Vec::new();
    let mut options = ChartOptions { legend: read_legend(chart)?, data_labels: read_labels(groups[0], read_kind(groups[0])?)?, ..Default::default() };
    for group in groups {
        let actual = read_kind(group)?;
        if kind == ChartKind::Combo && !matches!(actual, ChartKind::Column | ChartKind::Line | ChartKind::Area) { return Err(Error::Unsupported("combo group family".into())); }
        if read_labels(group, actual)? != options.data_labels { return Err(Error::Unsupported("different labels across combo groups".into())); }
        let mut axis = ChartAxis::Primary;
        if !actual.is_polar() {
            let ids: Vec<_> = group.children().filter(|node| node.has_tag_name((C, "axId"))).filter_map(|node| node.attribute("val")).collect();
            if ids.len() != 2 || ids[0] == ids[1] { return Err(Error::Unsupported("chart requires two distinct axes".into())); }
            let category = *axes.get(ids[0]).ok_or_else(|| Error::Unsupported("category/X axis target".into()))?;
            let value = *axes.get(ids[1]).ok_or_else(|| Error::Unsupported("value axis target".into()))?;
            if !category.has_tag_name((C, if actual.is_xy() { "valAx" } else { "catAx" })) || !value.has_tag_name((C, "valAx")) || child(category, C, "crossAx").and_then(|node| node.attribute("val")) != Some(ids[1]) || child(value, C, "crossAx").and_then(|node| node.attribute("val")) != Some(ids[0]) { return Err(Error::Unsupported("chart axes are not reciprocal category/value pairs".into())); }
            if child(value, C, "axPos").and_then(|node| node.attribute("val")) == Some(if actual.is_horizontal() { "t" } else { "r" }) { axis = ChartAxis::Secondary; }
            if axis_pairs.iter().any(|(existing, category, value)| *existing == axis && (*category != ids[0] || *value != ids[1])) { return Err(Error::Unsupported("more than one axis pair per side".into())); }
            if !axis_pairs.iter().any(|entry| *entry == (axis, ids[0], ids[1])) { axis_pairs.push((axis, ids[0], ids[1])); }
        }
        for entry in group.children().filter(|node| node.has_tag_name((C, "ser"))) {
            let required = |tag: &str| child(entry, C, tag).ok_or_else(|| Error::Unsupported(format!("chart {tag} missing")));
            let order: usize = required("order")?.attribute("val").and_then(|value| value.parse().ok()).ok_or_else(|| Error::Unsupported("series order".into()))?;
            let index: usize = required("idx")?.attribute("val").and_then(|value| value.parse().ok()).ok_or_else(|| Error::Unsupported("series index".into()))?;
            let current = cache(required(if actual.is_xy() { "xVal" } else { "cat" })?)?;
            if ordered.is_empty() { categories = current; } else if categories != current { return Err(Error::Unsupported("chart series use different X/category data".into())); }
            let values = numeric_cache(required(if actual.is_xy() { "yVal" } else { "val" })?)?;
            let name = child(entry, C, "tx").map(|node| cache(node).map(|values| values.first().cloned().or_else(|| child(node, C, "v").and_then(|node| node.text()).map(str::to_owned)).unwrap_or_else(|| "Series".into()))).transpose()?.unwrap_or_else(|| "Series".into());
            let shade = child(entry, C, "spPr").map(|node| child(node, A, "ln").map_or_else(|| read_color(node, "@accent1"), |line| read_color(line, "@accent1"))).unwrap_or_else(|| "@accent1".into());
            ordered.push((order, index, ChartSeries { name, values, color: shade, kind: (kind == ChartKind::Combo).then_some(actual), axis: (axis == ChartAxis::Secondary).then_some(axis), bubble_sizes: child(entry, C, "bubbleSize").map(numeric_cache).transpose()?, trendline: read_trendline(entry)?, error_bars: read_errors(entry, actual)? }));
        }
    }
    ordered.sort_by_key(|entry| entry.0);
    if ordered.windows(2).any(|entries| entries[0].0 == entries[1].0) || ordered.iter().map(|entry| entry.1).collect::<std::collections::BTreeSet<_>>().len() != ordered.len() { return Err(Error::Unsupported("duplicate chart series identity/order".into())); }
    let series: Vec<_> = ordered.into_iter().map(|entry| entry.2).collect();
    for (axis, category, value) in axis_pairs {
        let category_options = read_axis(axes[category], &AxisOptions::default(), None)?;
        let value_options = read_axis(axes[value], &default_axis(kind, &series, axis), Some(if kind.is_percent() { ("0%", "0") } else { ("General", "1") }))?;
        if axis == ChartAxis::Primary { options.category_axis = category_options; options.primary_axis = value_options; }
        else { options.secondary_axis = value_options; }
    }
    validate(kind, &categories, &series, &options)?;
    Ok((kind, categories, series, options))
}

fn text(writer: &mut XmlWriter, name: &str, value: &str) {
    writer.start_element(name);
    writer.write_text(&quick_xml::escape::escape(value));
    writer.end_element();
}

fn string_reference(writer: &mut XmlWriter, formula: &str, values: &[String]) {
    writer.start_element("c:strRef");
    text(writer, "c:f", formula);
    writer.start_element("c:strCache");
    empty(writer, "c:ptCount", &[("val", &values.len().to_string())]);
    for (index, value) in values.iter().enumerate() {
        writer.start_element("c:pt"); writer.write_attribute("idx", &index.to_string());
        text(writer, "c:v", value); writer.end_element();
    }
    writer.end_element(); writer.end_element();
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DataRole { Values, Bubble, Plus, Minus }

struct NumericColumn<'a> { series: usize, role: DataRole, name: String, values: &'a [f64] }

fn numeric_columns(series: &[ChartSeries]) -> Vec<NumericColumn<'_>> {
    let mut columns: Vec<_> = series.iter().enumerate().map(|(index, entry)| NumericColumn { series: index, role: DataRole::Values, name: entry.name.clone(), values: &entry.values }).collect();
    for (index, entry) in series.iter().enumerate() {
        if let Some(values) = &entry.bubble_sizes { columns.push(NumericColumn { series: index, role: DataRole::Bubble, name: format!("{} size", entry.name), values }); }
    }
    for (index, entry) in series.iter().enumerate() {
        if let Some(errors) = &entry.error_bars {
            for (role, values, suffix) in [(DataRole::Plus, &errors.plus, "error plus"), (DataRole::Minus, &errors.minus, "error minus")] {
                if let Some(values) = values { columns.push(NumericColumn { series: index, role, name: format!("{} {suffix}", entry.name), values }); }
            }
        }
    }
    columns
}

fn column_name(mut index: usize) -> String {
    let mut letters = Vec::new();
    loop { letters.push(char::from(b'A' + (index % 26) as u8)); if index < 26 { break; } index = index / 26 - 1; }
    letters.into_iter().rev().collect()
}

fn numeric_reference(writer: &mut XmlWriter, formula: &str, values: impl Iterator<Item = String>) {
    let values: Vec<_> = values.collect();
    writer.start_element("c:numRef"); text(writer, "c:f", formula);
    writer.start_element("c:numCache"); text(writer, "c:formatCode", "General");
    empty(writer, "c:ptCount", &[("val", &values.len().to_string())]);
    for (index, value) in values.iter().enumerate() { writer.start_element("c:pt"); writer.write_attribute("idx", &index.to_string()); text(writer, "c:v", value); writer.end_element(); }
    writer.end_element(); writer.end_element();
}

fn column_reference(writer: &mut XmlWriter, columns: &[NumericColumn<'_>], index: usize, role: DataRole) {
    if let Some((position, column)) = columns.iter().enumerate().find(|(_, column)| column.series == index && column.role == role) {
        let letter = column_name(position + 1);
        numeric_reference(writer, &format!("Sheet1!${letter}$2:${letter}${}", column.values.len() + 1), column.values.iter().map(ToString::to_string));
    }
}

fn labels(writer: &mut XmlWriter, labels: &DataLabels, kind: ChartKind, legacy_positions: bool) {
    writer.start_element("c:dLbls");
    if let Some(format) = &labels.number_format { empty(writer, "c:numFmt", &[("formatCode", format), ("sourceLinked", "0")]); }
    if legacy_positions || !automatic_label_position(kind) { if let Some(position) = labels.position { empty(writer, "c:dLblPos", &[("val", match position { LabelPosition::Center => "ctr", LabelPosition::InsideEnd => "inEnd", LabelPosition::OutsideEnd => "outEnd", LabelPosition::BestFit => "bestFit" })]); } }
    for (name, show) in [("c:showLegendKey", false), ("c:showVal", labels.show_value), ("c:showCatName", labels.show_category_name), ("c:showSerName", labels.show_series_name), ("c:showPercent", labels.show_percent), ("c:showBubbleSize", false)] { empty(writer, name, &[("val", if show { "1" } else { "0" })]); }
    if !legacy_positions && automatic_label_position(kind) && labels.position == Some(LabelPosition::Center) {
        writer.start_element("c:extLst");
        writer.start_element("c:ext"); writer.write_attribute("uri", LABEL_POSITION_NS);
        empty(writer, "ais:position", &[("xmlns:ais", LABEL_POSITION_NS), ("val", "center")]);
        writer.end_element(); writer.end_element();
    }
    writer.end_element();
}

fn statistics(writer: &mut XmlWriter, entry: &ChartSeries, columns: &[NumericColumn<'_>], index: usize, kind: ChartKind) {
    if let Some(trend) = &entry.trendline {
        writer.start_element("c:trendline");
        empty(writer, "c:trendlineType", &[("val", match trend.kind { TrendlineKind::Linear => "linear", TrendlineKind::Exponential => "exp", TrendlineKind::Logarithmic => "log", TrendlineKind::Polynomial => "poly", TrendlineKind::Power => "power", TrendlineKind::MovingAverage => "movingAvg" })]);
        if let Some(order) = trend.order { empty(writer, "c:order", &[("val", &order.to_string())]); }
        if let Some(period) = trend.period { empty(writer, "c:period", &[("val", &period.to_string())]); }
        for (name, value) in [("c:forward", trend.forward), ("c:backward", trend.backward), ("c:intercept", trend.intercept)] { if let Some(value) = value { empty(writer, name, &[("val", &value.to_string())]); } }
        empty(writer, "c:dispRSqr", &[("val", if trend.display_r_squared { "1" } else { "0" })]);
        empty(writer, "c:dispEq", &[("val", if trend.display_equation { "1" } else { "0" })]);
        writer.end_element();
    }
    if let Some(errors) = &entry.error_bars {
        writer.start_element("c:errBars");
        let direction = if errors.direction == ErrorBarDirection::X || kind.is_horizontal() { "x" } else { "y" };
        empty(writer, "c:errDir", &[("val", direction)]);
        empty(writer, "c:errBarType", &[("val", match errors.bar_type { ErrorBarType::Both => "both", ErrorBarType::Plus => "plus", ErrorBarType::Minus => "minus" })]);
        empty(writer, "c:errValType", &[("val", match errors.kind { ErrorBarKind::FixedValue => "fixedVal", ErrorBarKind::Percentage => "percentage", ErrorBarKind::StandardDeviation => "stdDev", ErrorBarKind::StandardError => "stdErr", ErrorBarKind::Custom => "cust" })]);
        empty(writer, "c:noEndCap", &[("val", "0")]);
        for (name, role, values) in [("c:plus", DataRole::Plus, &errors.plus), ("c:minus", DataRole::Minus, &errors.minus)] {
            if values.is_some() { writer.start_element(name); column_reference(writer, columns, index, role); writer.end_element(); }
        }
        if let Some(value) = errors.value { empty(writer, "c:val", &[("val", &value.to_string())]); }
        writer.end_element();
    }
}

pub(crate) fn default_axis(kind: ChartKind, series: &[ChartSeries], axis: ChartAxis) -> AxisOptions {
    let entries: Vec<_> = series.iter().filter(|entry| entry.axis.unwrap_or_default() == axis).collect();
    let mut options = AxisOptions::default();
    if kind.is_percent() { options.max = Some(1.0); }
    if kind.is_bar() || kind == ChartKind::Area || (kind == ChartKind::Combo && entries.iter().any(|entry| matches!(entry.kind, Some(ChartKind::Column | ChartKind::Area)))) {
        if entries.iter().flat_map(|entry| &entry.values).all(|value| *value >= 0.0) { options.min = Some(0.0); }
        else if entries.iter().flat_map(|entry| &entry.values).all(|value| *value <= 0.0) { options.max = Some(0.0); }
    }
    options
}

pub(crate) fn chart(kind: ChartKind, categories: &[String], series: &[ChartSeries], options: &ChartOptions) -> Vec<u8> {
    chart_with_label_positions(kind, categories, series, options, false)
}

pub(crate) fn legacy_label_position_chart(kind: ChartKind, categories: &[String], series: &[ChartSeries], options: &ChartOptions) -> Vec<u8> {
    chart_with_label_positions(kind, categories, series, options, true)
}

fn chart_with_label_positions(kind: ChartKind, categories: &[String], series: &[ChartSeries], options: &ChartOptions, legacy_positions: bool) -> Vec<u8> {
    if kind.is_extended() { return crate::chart_extended::chart(kind, categories, series, options); }
    let columns = numeric_columns(series);
    let mut groups: Vec<(ChartKind, ChartAxis, Vec<usize>)> = Vec::new();
    for (index, entry) in series.iter().enumerate() {
        let actual = entry.kind.unwrap_or(kind); let axis = entry.axis.unwrap_or_default();
        if let Some(group) = groups.iter_mut().find(|(group_kind, group_axis, _)| *group_kind == actual && *group_axis == axis) { group.2.push(index); }
        else { groups.push((actual, axis, vec![index])); }
    }
    xml("c:chartSpace", |writer| {
        writer.write_attribute("xmlns:c", CHART_NS);
        empty(writer, "c:lang", &[("val", "ja-JP")]);
        writer.start_element("c:chart");
        empty(writer, "c:autoTitleDeleted", &[("val", "1")]);
        if kind.is_3d() { writer.start_element("c:view3D"); empty(writer, "c:rotX", &[("val", "15")]); empty(writer, "c:rotY", &[("val", "20")]); empty(writer, "c:rAngAx", &[("val", "1")]); writer.end_element(); }
        writer.start_element("c:plotArea"); empty(writer, "c:layout", &[]);
        for (kind, axis, indexes) in &groups {
        let kind = *kind;
        writer.start_element(match kind { ChartKind::Line => "c:lineChart", ChartKind::Area => "c:areaChart", ChartKind::Pie => "c:pieChart", ChartKind::Doughnut => "c:doughnutChart", ChartKind::Scatter => "c:scatterChart", ChartKind::Bubble => "c:bubbleChart", ChartKind::Radar | ChartKind::RadarFilled => "c:radarChart", ChartKind::Column3d | ChartKind::Bar3d => "c:bar3DChart", ChartKind::Pie3d => "c:pie3DChart", _ => "c:barChart" });
        if kind.is_bar() { empty(writer, "c:barDir", &[("val", if kind.is_horizontal() { "bar" } else { "col" })]); }
        if !kind.is_polar() && !kind.is_xy() && !kind.is_radar() { empty(writer, "c:grouping", &[("val", if kind.is_percent() { "percentStacked" } else if matches!(kind, ChartKind::StackedColumn | ChartKind::StackedBar) { "stacked" } else if kind.is_bar() { "clustered" } else { "standard" })]); }
        if kind == ChartKind::Scatter { empty(writer, "c:scatterStyle", &[("val", "marker")]); }
        if kind.is_radar() { empty(writer, "c:radarStyle", &[("val", if kind == ChartKind::RadarFilled { "filled" } else { "marker" })]); }
        empty(writer, "c:varyColors", &[("val", if kind.is_polar() { "1" } else { "0" })]);
        for index in indexes.iter().copied() {
            let entry = &series[index]; let column = column_name(index + 1);
            writer.start_element("c:ser");
            empty(writer, "c:idx", &[("val", &index.to_string())]);
            empty(writer, "c:order", &[("val", &index.to_string())]);
            writer.start_element("c:tx"); string_reference(writer, &format!("Sheet1!${column}$1"), std::slice::from_ref(&entry.name)); writer.end_element();
            writer.start_element("c:spPr");
            if matches!(kind, ChartKind::Line | ChartKind::Scatter | ChartKind::Radar) {
                writer.start_element("a:ln"); writer.write_attribute("w", "28575"); color(writer, &entry.color); writer.end_element();
            } else { color(writer, &entry.color); }
            writer.end_element();
            if matches!(kind, ChartKind::Line | ChartKind::Scatter | ChartKind::Radar) { writer.start_element("c:marker"); empty(writer, "c:symbol", &[("val", "circle")]); empty(writer, "c:size", &[("val", "5")]); writer.end_element(); }
            else if kind.is_bar() || kind == ChartKind::Bubble { empty(writer, "c:invertIfNegative", &[("val", "0")]); }
            if kind.is_polar() {
                for position in 0..categories.len() { writer.start_element("c:dPt"); empty(writer, "c:idx", &[("val", &position.to_string())]); writer.start_element("c:spPr"); color(writer, &format!("@accent{}", position % 6 + 1)); writer.end_element(); writer.end_element(); }
            }
            statistics(writer, entry, &columns, index, kind);
            if kind.is_xy() {
                writer.start_element("c:xVal"); numeric_reference(writer, &format!("Sheet1!$A$2:$A${}", categories.len() + 1), categories.iter().cloned()); writer.end_element();
            } else { writer.start_element("c:cat"); string_reference(writer, &format!("Sheet1!$A$2:$A${}", categories.len() + 1), categories); writer.end_element(); }
            writer.start_element(if kind.is_xy() { "c:yVal" } else { "c:val" }); column_reference(writer, &columns, index, DataRole::Values); writer.end_element();
            if kind == ChartKind::Bubble { writer.start_element("c:bubbleSize"); column_reference(writer, &columns, index, DataRole::Bubble); writer.end_element(); empty(writer, "c:bubble3D", &[("val", "0")]); }
            if kind == ChartKind::Line { empty(writer, "c:smooth", &[("val", "0")]); }
            writer.end_element();
        }
        if let Some(data_labels) = &options.data_labels { labels(writer, data_labels, kind, legacy_positions); }
        if kind.is_bar() {
            empty(writer, "c:gapWidth", &[("val", "90")]);
            if kind.is_3d() { empty(writer, "c:gapDepth", &[("val", "150")]); empty(writer, "c:shape", &[("val", "box")]); }
            else { empty(writer, "c:overlap", &[("val", if kind.is_percent() || matches!(kind, ChartKind::StackedColumn | ChartKind::StackedBar) { "100" } else { "0" })]); }
        }
        if kind == ChartKind::Bubble { empty(writer, "c:bubbleScale", &[("val", "100")]); empty(writer, "c:showNegBubbles", &[("val", "0")]); empty(writer, "c:sizeRepresents", &[("val", "area")]); }
        if kind.is_polar() { if kind != ChartKind::Pie3d { empty(writer, "c:firstSliceAng", &[("val", "0")]); } if kind == ChartKind::Doughnut { empty(writer, "c:holeSize", &[("val", "50")]); } }
        else { empty(writer, "c:axId", &[("val", if *axis == ChartAxis::Primary { "1" } else { "3" })]); empty(writer, "c:axId", &[("val", if *axis == ChartAxis::Primary { "2" } else { "4" })]); }
        writer.end_element();
        }
        for axis in [ChartAxis::Primary, ChartAxis::Secondary] {
        if axis == ChartAxis::Secondary && !groups.iter().any(|(_, group_axis, _)| *group_axis == axis) { continue; }
        let secondary = axis == ChartAxis::Secondary;
        for (tag, id, cross, position, value_axis) in [(if kind.is_xy() { "c:valAx" } else { "c:catAx" }, if secondary { "3" } else { "1" }, if secondary { "4" } else { "2" }, if secondary { "t" } else if kind.is_horizontal() { "l" } else { "b" }, false), ("c:valAx", if secondary { "4" } else { "2" }, if secondary { "3" } else { "1" }, if secondary { "r" } else if kind.is_horizontal() { "b" } else { "l" }, true)] {
            if kind.is_polar() { break; }
            let custom = if !value_axis { &options.category_axis } else if secondary { &options.secondary_axis } else { &options.primary_axis };
            let defaults = if value_axis && custom.log_base.is_none() { default_axis(kind, series, axis) } else { AxisOptions::default() };
            writer.start_element(tag); empty(writer, "c:axId", &[("val", id)]);
            writer.start_element("c:scaling");
            if let Some(base) = custom.log_base { empty(writer, "c:logBase", &[("val", &base.to_string())]); }
            empty(writer, "c:orientation", &[("val", if custom.reverse { "maxMin" } else { "minMax" })]);
            if let Some(max) = custom.max.or(defaults.max) { empty(writer, "c:max", &[("val", &max.to_string())]); }
            if let Some(min) = custom.min.or(defaults.min) { empty(writer, "c:min", &[("val", &min.to_string())]); }
            writer.end_element();
            empty(writer, "c:delete", &[("val", if secondary && !value_axis { "1" } else { "0" })]); empty(writer, "c:axPos", &[("val", position)]);
            if value_axis && !secondary {
                writer.start_element("c:majorGridlines"); writer.start_element("c:spPr"); writer.start_element("a:ln"); writer.write_attribute("w", "6350"); color(writer, "DDE4E1"); writer.end_element(); writer.end_element(); writer.end_element();
            }
            if value_axis || custom.number_format.is_some() { empty(writer, "c:numFmt", &[("formatCode", custom.number_format.as_deref().unwrap_or(if kind.is_percent() { "0%" } else { "General" })), ("sourceLinked", if custom.number_format.is_some() || kind.is_percent() { "0" } else { "1" })]); }
            empty(writer, "c:majorTickMark", &[("val", "none")]); empty(writer, "c:minorTickMark", &[("val", "none")]);
            empty(writer, "c:tickLblPos", &[("val", "nextTo")]); empty(writer, "c:crossAx", &[("val", cross)]); empty(writer, "c:crosses", &[("val", "autoZero")]);
            if tag == "c:catAx" { empty(writer, "c:auto", &[("val", "1")]); empty(writer, "c:lblAlgn", &[("val", "ctr")]); empty(writer, "c:lblOffset", &[("val", "100")]); }
            else { empty(writer, "c:crossBetween", &[("val", "between")]); if let Some(unit) = custom.major_unit { empty(writer, "c:majorUnit", &[("val", &unit.to_string())]); } if let Some(unit) = custom.minor_unit { empty(writer, "c:minorUnit", &[("val", &unit.to_string())]); } }
            writer.end_element();
        }
        }
        writer.end_element();
        if options.legend != Some(LegendPosition::Hidden) { writer.start_element("c:legend"); empty(writer, "c:legendPos", &[("val", match options.legend.unwrap_or(LegendPosition::Bottom) { LegendPosition::Bottom | LegendPosition::Hidden => "b", LegendPosition::Top => "t", LegendPosition::Left => "l", LegendPosition::Right => "r", LegendPosition::TopRight => "tr" })]); empty(writer, "c:overlay", &[("val", "0")]); writer.end_element(); }
        empty(writer, "c:plotVisOnly", &[("val", "1")]); empty(writer, "c:dispBlanksAs", &[("val", "gap")]); writer.end_element();
        writer.start_element("c:externalData"); writer.write_attribute("r:id", "rIdWorkbook"); empty(writer, "c:autoUpdate", &[("val", "0")]); writer.end_element();
    })
}

fn string_cell(writer: &mut XmlWriter, address: &str, value: &str) {
    writer.start_element("c"); writer.write_attribute("r", address); writer.write_attribute("t", "inlineStr");
    writer.start_element("is"); writer.start_element("t"); writer.write_attribute("xml:space", "preserve");
    writer.write_text(&quick_xml::escape::escape(value)); writer.end_element(); writer.end_element(); writer.end_element();
}

pub(crate) fn workbook(kind: ChartKind, categories: &[String], series: &[ChartSeries], options: &ChartOptions) -> Result<Vec<u8>> {
    let columns = numeric_columns(series);
    let mut parts = BTreeMap::new();
    parts.insert("[Content_Types].xml".into(), br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>"#.to_vec());
    parts.insert("_rels/.rels".into(), br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#.to_vec());
    parts.insert("xl/workbook.xml".into(), xml("workbook", |writer| {
        writer.write_attribute("xmlns", SHEET_NS); writer.start_element("sheets"); empty(writer, "sheet", &[("name", "Sheet1"), ("sheetId", "1"), ("r:id", "rId1")]); writer.end_element();
    }));
    parts.insert("xl/_rels/workbook.xml.rels".into(), br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#.to_vec());
    parts.insert("xl/worksheets/sheet1.xml".into(), if options.histogram.is_some() || options.box_whisker.is_some() || options.hierarchy.is_some() { crate::chart_extended::worksheet(categories, series, options) } else { xml("worksheet", |writer| {
        writer.write_attribute("xmlns", SHEET_NS); writer.start_element("sheetData");
        writer.start_element("row"); writer.write_attribute("r", "1"); string_cell(writer, "A1", "Category");
        for (index, column) in columns.iter().enumerate() { string_cell(writer, &format!("{}1", column_name(index + 1)), &column.name); }
        writer.end_element();
        for (index, category) in categories.iter().enumerate() {
            let row = index + 2;
            writer.start_element("row"); writer.write_attribute("r", &row.to_string());
            if kind.is_xy() { writer.start_element("c"); writer.write_attribute("r", &format!("A{row}")); text(writer, "v", category); writer.end_element(); }
            else { string_cell(writer, &format!("A{row}"), category); }
            for (position, column) in columns.iter().enumerate() {
                writer.start_element("c"); writer.write_attribute("r", &format!("{}{row}", column_name(position + 1))); text(writer, "v", &column.values[index].to_string()); writer.end_element();
            }
            writer.end_element();
        }
        writer.end_element();
    }) });
    Package::from_parts(parts)?.save()
}
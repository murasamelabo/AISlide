use crate::{model::{ChartKind, ChartOptions, ChartSeries, chart_format::LegendPosition}, native::child, pptx::{color, empty, xml}, Error, Result};
use roxmltree::Node;
use std::collections::BTreeSet;
use xmlwriter::XmlWriter;
use crate::model::chart_format::{HistogramBinning, IntervalClosed, QuartileMethod, ParentLabelLayout, HistogramOptions, BoxWhiskerOptions, HierarchyOptions};

pub(crate) const NS: &str = "http://schemas.microsoft.com/office/drawing/2014/chartex";
pub(crate) const RELATIONSHIP: &str = "http://schemas.microsoft.com/office/2014/relationships/chartEx";
pub(crate) const CONTENT_TYPE: &str = "application/vnd.ms-office.chartex+xml";

pub(crate) fn style() -> Vec<u8> {
    xml("cs:chartStyle", |writer| {
        writer.write_attribute("xmlns:cs", "http://schemas.microsoft.com/office/drawing/2012/chartStyle");
        writer.write_attribute("id", "1");
        for tag in ["axisTitle", "categoryAxis", "chartArea", "dataLabel", "dataPoint", "dataPoint3D", "dataPointLine", "dataPointMarker", "dataPointWireframe", "dataTable", "downBar", "dropLine", "errorBar", "floor", "gridlineMajor", "gridlineMinor", "hiLoLine", "leaderLine", "legend", "plotArea", "plotArea3D", "seriesAxis", "seriesLine", "title", "trendline", "trendlineLabel", "upBar", "valueAxis", "wall"] {
            writer.start_element(&format!("cs:{tag}"));
            empty(writer, "cs:lnRef", &[("idx", "0")]);
            writer.start_element("cs:fillRef"); writer.write_attribute("idx", "0");
            if tag.starts_with("dataPoint") { empty(writer, "cs:styleClr", &[("val", "auto")]); }
            writer.end_element();
            empty(writer, "cs:effectRef", &[("idx", "0")]);
            writer.start_element("cs:fontRef"); writer.write_attribute("idx", "minor"); empty(writer, "a:schemeClr", &[("val", "dk1")]); writer.end_element();
            if tag.starts_with("dataPoint") {
                writer.start_element("cs:spPr"); color(writer, "@phClr"); writer.end_element();
            }
            empty(writer, "cs:defRPr", &[("sz", "1400")]);
            writer.end_element();
        }
    })
}

pub(crate) fn color_style() -> Vec<u8> {
    xml("cs:colorStyle", |writer| {
        writer.write_attribute("xmlns:cs", "http://schemas.microsoft.com/office/drawing/2012/chartStyle");
        writer.write_attribute("meth", "cycle"); writer.write_attribute("id", "1");
        for slot in ["accent1", "accent2", "accent3", "accent4", "accent5", "accent6"] { empty(writer, "a:schemeClr", &[("val", slot)]); }
    })
}

pub(crate) fn validate(kind: ChartKind, categories: &[String], series: &[ChartSeries], options: &ChartOptions) -> Result<()> {
    let invalid = |message: &str| Error::Invalid(message.into());
    if matches!(kind, ChartKind::Histogram | ChartKind::BoxWhisker) {
        if (kind == ChartKind::BoxWhisker && categories.is_empty()) || categories.len() > 32 || series.len() != 1 || !series[0].values.is_empty() { return Err(invalid("statistical charts require bounded groups, one series and empty values; edit raw samples")); }
        for label in categories { crate::model::valid_text(label, 80)?; if label.trim().is_empty() { return Err(invalid("sample group labels cannot be empty")); } }
        if categories.iter().collect::<BTreeSet<_>>().len() != categories.len() { return Err(invalid("sample group labels must be unique")); }
        crate::model::valid_text(&series[0].name, 80)?; crate::model::valid_color(&series[0].color)?;
    } else { crate::model::validate_chart(categories, series)?; }
    if (kind == ChartKind::Histogram) != options.histogram.is_some() || (kind == ChartKind::BoxWhisker) != options.box_whisker.is_some() || matches!(kind, ChartKind::Treemap | ChartKind::Sunburst) != options.hierarchy.is_some() { return Err(invalid("chart family requires exactly its typed data options")); }
    if kind != ChartKind::Waterfall && options.waterfall_totals.is_some() { return Err(invalid("waterfall_totals requires waterfall")); }
    if series.len() != 1 || series.iter().any(|entry| entry.kind.is_some() || entry.axis.is_some() || entry.bubble_sizes.is_some() || entry.trendline.is_some() || entry.error_bars.is_some()) {
        return Err(invalid("chartEx requires one plain series without combo, bubble, trendline or error-bar options"));
    }
    if !options.primary_axis.is_default() || !options.secondary_axis.is_default() || !options.category_axis.is_default() || options.data_labels.is_some() {
        return Err(invalid("chartEx custom axes and data labels are not supported"));
    }
    if kind == ChartKind::Funnel {
        if options.waterfall_totals.is_some() || series[0].values.iter().any(|value| *value <= 0.0) { return Err(invalid("funnel requires strictly positive values and no waterfall_totals")); }
    } else if kind == ChartKind::Waterfall {
        let totals = options.waterfall_totals.as_ref().ok_or_else(|| invalid("waterfall requires explicit waterfall_totals indices (empty means all changes)"))?;
        if totals.len() > categories.len() || totals.iter().any(|index| *index as usize >= categories.len()) || totals.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(invalid("waterfall_totals must be unique increasing zero-based category indices"));
        }
    } else if let Some(histogram) = &options.histogram {
        validate_samples(&histogram.samples, 1)?;
        if !categories.is_empty() { return Err(invalid("histogram requires empty categories; raw numerical samples have no category dimension")); }
        let low = histogram.samples.iter().copied().fold(f64::INFINITY, f64::min);
        let high = histogram.samples.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        if [histogram.underflow, histogram.overflow].into_iter().flatten().any(|value| !value.is_finite() || value.abs() > 1e15) || histogram.underflow.zip(histogram.overflow).is_some_and(|(lower, upper)| lower >= upper) { return Err(invalid("histogram thresholds must be finite ordered values in +/-1e15")); }
        match histogram.binning {
            HistogramBinning::Count { count } if (1..=128).contains(&count) => {},
            HistogramBinning::Width { width } if width.is_finite() && width > 0.0 && width <= 1e15 && ((histogram.overflow.unwrap_or(high) - histogram.underflow.unwrap_or(low)).max(0.0) / width).ceil() <= 128.0 => {},
            _ => return Err(invalid("histogram requires count 1..128 or positive width producing at most 128 regular bins")),
        }
    } else if let Some(statistics) = &options.box_whisker {
        if statistics.samples.len() != categories.len() || statistics.samples.iter().map(Vec::len).sum::<usize>() > 4096 { return Err(invalid("box samples must match categories with at most 4096 total samples")); }
        for group in &statistics.samples { validate_samples(group, 4)?; }
    } else if let Some(hierarchy) = &options.hierarchy {
        let depth = hierarchy.paths.first().map_or(0, Vec::len);
        if !(2..=4).contains(&depth) || hierarchy.paths.len() != categories.len() || hierarchy.paths.iter().any(|path| path.len() != depth) || hierarchy.paths.iter().collect::<BTreeSet<_>>().len() != hierarchy.paths.len() { return Err(invalid("hierarchy requires 1..32 unique leaf paths with equal depth 2..4")); }
        for (path, category) in hierarchy.paths.iter().zip(categories) {
            for label in path { crate::model::valid_text(label, 80)?; if label.trim().is_empty() { return Err(invalid("hierarchy path labels cannot be empty")); } }
            if path.last() != Some(category) { return Err(invalid("hierarchy category must equal its leaf label")); }
        }
        if series[0].values.iter().any(|value| *value <= 0.0) { return Err(invalid("hierarchy leaf values must be strictly positive")); }
        if kind == ChartKind::Sunburst && hierarchy.parent_labels.is_some() { return Err(invalid("parent_labels applies only to treemap")); }
    } else { return Err(invalid("unimplemented chartEx family")); }
    Ok(())
}

fn validate_samples(samples: &[f64], minimum: usize) -> Result<()> {
    if samples.len() < minimum || samples.len() > 4096 || samples.iter().any(|value| !value.is_finite() || value.abs() > 1e15) { return Err(Error::Invalid(format!("samples require {minimum}..4096 finite values in +/-1e15"))); }
    Ok(())
}

fn text(writer: &mut XmlWriter, tag: &str, value: &str) {
    writer.start_element(tag);
    writer.write_text(&quick_xml::escape::escape(value));
    writer.end_element();
}

fn source_rows(categories: &[String], series: &[ChartSeries], options: &ChartOptions) -> (Vec<Vec<String>>, Vec<f64>) {
    if let Some(histogram) = &options.histogram {
        return (histogram.samples.iter().map(|_| vec![series[0].name.clone()]).collect(), histogram.samples.clone());
    }
    if let Some(statistics) = &options.box_whisker {
        return (categories.iter().zip(&statistics.samples).flat_map(|(label, samples)| samples.iter().map(move |_| vec![label.clone()])).collect(), statistics.samples.iter().flatten().copied().collect());
    }
    (options.hierarchy.as_ref().map_or_else(|| categories.iter().map(|label| vec![label.clone()]).collect(), |hierarchy| hierarchy.paths.clone()), series[0].values.clone())
}

pub(crate) fn worksheet(categories: &[String], series: &[ChartSeries], options: &ChartOptions) -> Vec<u8> {
    let (paths, values) = source_rows(categories, series, options);
    let depth = paths[0].len();
    let string_cell = |writer: &mut XmlWriter, address: &str, value: &str| {
        writer.start_element("c"); writer.write_attribute("r", address); writer.write_attribute("t", "inlineStr");
        writer.start_element("is"); writer.start_element("t"); writer.write_attribute("xml:space", "preserve"); writer.write_text(&quick_xml::escape::escape(value)); writer.end_element(); writer.end_element(); writer.end_element();
    };
    xml("worksheet", |writer| {
        writer.write_attribute("xmlns", "http://schemas.openxmlformats.org/spreadsheetml/2006/main"); writer.start_element("sheetData");
        writer.start_element("row"); writer.write_attribute("r", "1");
        for column in 0..depth { string_cell(writer, &format!("{}1", char::from(b'A' + column as u8)), if depth == 1 { "Category" } else { "Hierarchy" }); }
        string_cell(writer, &format!("{}1", char::from(b'A' + depth as u8)), &series[0].name); writer.end_element();
        for (index, path) in paths.iter().enumerate() {
            let row = index + 2; writer.start_element("row"); writer.write_attribute("r", &row.to_string());
            for (column, label) in path.iter().enumerate() { string_cell(writer, &format!("{}{row}", char::from(b'A' + column as u8)), label); }
            writer.start_element("c"); writer.write_attribute("r", &format!("{}{row}", char::from(b'A' + depth as u8))); text(writer, "v", &values[index].to_string()); writer.end_element(); writer.end_element();
        }
        writer.end_element();
    })
}

pub(crate) fn chart(kind: ChartKind, categories: &[String], series: &[ChartSeries], options: &ChartOptions) -> Vec<u8> {
    chart_with_boundaries(kind, categories, series, options, true)
}

pub(crate) fn legacy_sunburst_chart(categories: &[String], series: &[ChartSeries], options: &ChartOptions) -> Vec<u8> {
    chart_with_boundaries(ChartKind::Sunburst, categories, series, options, false)
}

fn chart_with_boundaries(kind: ChartKind, categories: &[String], series: &[ChartSeries], options: &ChartOptions, boundaries: bool) -> Vec<u8> {
    let (paths, values) = source_rows(categories, series, options);
    let depth = paths[0].len();
    let numeric_column = char::from(b'A' + depth as u8);
    xml("cx:chartSpace", |writer| {
        writer.write_attribute("xmlns:cx", NS);
        writer.start_element("cx:chartData");
        empty(writer, "cx:externalData", &[("r:id", "rIdWorkbook"), ("cx:autoUpdate", "0")]);
        writer.start_element("cx:data"); writer.write_attribute("id", "0");
        if kind != ChartKind::Histogram {
        writer.start_element("cx:strDim"); writer.write_attribute("type", "cat");
        text(writer, "cx:f", &format!("Sheet1!$A$2:${}${}", char::from(b'A' + depth as u8 - 1), paths.len() + 1));
        for level in (0..depth).rev() {
            writer.start_element("cx:lvl"); writer.write_attribute("ptCount", &paths.len().to_string());
            for (index, path) in paths.iter().enumerate() {
                writer.start_element("cx:pt"); writer.write_attribute("idx", &index.to_string());
                writer.write_text(&quick_xml::escape::escape(&path[level])); writer.end_element();
            }
            writer.end_element();
        }
        writer.end_element();
        }
        writer.start_element("cx:numDim"); writer.write_attribute("type", if options.hierarchy.is_some() { "size" } else { "val" });
        text(writer, "cx:f", &format!("Sheet1!${numeric_column}$2:${numeric_column}${}", paths.len() + 1));
        writer.start_element("cx:lvl"); writer.write_attribute("ptCount", &values.len().to_string()); writer.write_attribute("formatCode", "General");
        for (index, value) in values.iter().enumerate() {
            writer.start_element("cx:pt"); writer.write_attribute("idx", &index.to_string()); writer.write_text(&value.to_string()); writer.end_element();
        }
        writer.end_element(); writer.end_element(); writer.end_element(); writer.end_element();
        writer.start_element("cx:chart"); writer.start_element("cx:plotArea"); writer.start_element("cx:plotAreaRegion");
        writer.start_element("cx:series"); writer.write_attribute("layoutId", match kind { ChartKind::Funnel => "funnel", ChartKind::Histogram => "clusteredColumn", ChartKind::BoxWhisker => "boxWhisker", ChartKind::Treemap => "treemap", ChartKind::Sunburst => "sunburst", _ => "waterfall" });
        writer.start_element("cx:tx"); writer.start_element("cx:txData"); text(writer, "cx:f", &format!("Sheet1!${numeric_column}$1")); text(writer, "cx:v", &series[0].name); writer.end_element(); writer.end_element();
        writer.start_element("cx:spPr"); color(writer, &series[0].color);
        if boundaries && kind == ChartKind::Sunburst {
            writer.start_element("a:ln"); writer.write_attribute("w", "19050"); color(writer, "@lt1"); writer.end_element();
        }
        writer.end_element();
        empty(writer, "cx:dataId", &[("val", "0")]);
        if let Some(totals) = &options.waterfall_totals {
            writer.start_element("cx:layoutPr"); writer.start_element("cx:subtotals");
            for index in totals { empty(writer, "cx:idx", &[("val", &index.to_string())]); }
            writer.end_element(); writer.end_element();
        }
        if options.histogram.is_some() || options.box_whisker.is_some() || options.hierarchy.as_ref().is_some_and(|hierarchy| hierarchy.parent_labels.is_some()) {
            writer.start_element("cx:layoutPr");
            if let Some(layout) = options.hierarchy.as_ref().and_then(|hierarchy| hierarchy.parent_labels) { empty(writer, "cx:parentLabelLayout", &[("val", match layout { ParentLabelLayout::None => "none", ParentLabelLayout::Banner => "banner", ParentLabelLayout::Overlapping => "overlapping" })]); }
            if let Some(statistics) = &options.box_whisker { empty(writer, "cx:visibility", &[("meanLine", if statistics.mean_line { "1" } else { "0" }), ("meanMarker", if statistics.mean_marker { "1" } else { "0" }), ("nonoutliers", if statistics.nonoutliers { "1" } else { "0" }), ("outliers", if statistics.outliers { "1" } else { "0" })]); }
            if let Some(histogram) = &options.histogram {
                writer.start_element("cx:binning"); writer.write_attribute("intervalClosed", if histogram.interval_closed == IntervalClosed::Left { "l" } else { "r" });
                if let Some(value) = histogram.underflow { writer.write_attribute("underflow", &value.to_string()); }
                if let Some(value) = histogram.overflow { writer.write_attribute("overflow", &value.to_string()); }
                match histogram.binning { HistogramBinning::Count { count } => empty(writer, "cx:binCount", &[("val", &count.to_string())]), HistogramBinning::Width { width } => empty(writer, "cx:binSize", &[("val", &width.to_string())]) }
                writer.end_element();
            }
            if let Some(statistics) = &options.box_whisker { empty(writer, "cx:statistics", &[("quartileMethod", if statistics.quartile_method == QuartileMethod::Inclusive { "inclusive" } else { "exclusive" })]); }
            writer.end_element();
        }
        writer.end_element(); writer.end_element();
        if matches!(kind, ChartKind::Waterfall | ChartKind::Histogram | ChartKind::BoxWhisker) {
            writer.start_element("cx:axis"); writer.write_attribute("id", "0"); empty(writer, "cx:catScaling", &[]); empty(writer, "cx:tickLabels", &[]); writer.end_element();
            writer.start_element("cx:axis"); writer.write_attribute("id", "1"); empty(writer, "cx:valScaling", &[]); empty(writer, "cx:majorGridlines", &[]); empty(writer, "cx:tickLabels", &[]); writer.end_element();
        }
        writer.end_element();
        if options.legend != Some(LegendPosition::Hidden) {
            let position = match options.legend.unwrap_or(LegendPosition::Bottom) { LegendPosition::Bottom | LegendPosition::Hidden => "b", LegendPosition::Top => "t", LegendPosition::Left => "l", LegendPosition::Right | LegendPosition::TopRight => "r" };
            empty(writer, "cx:legend", &[("pos", position), ("align", if options.legend == Some(LegendPosition::TopRight) { "max" } else { "ctr" }), ("overlay", "0")]);
        }
        writer.end_element();
    })
}

fn required<'a, 'input>(node: Node<'a, 'input>, tag: &str) -> Result<Node<'a, 'input>> {
    let mut matches = node.children().filter(|node| node.has_tag_name((NS, tag)));
    let value = matches.next().ok_or_else(|| Error::Unsupported(format!("chartEx {tag} missing")))?;
    if matches.next().is_some() { return Err(Error::Unsupported(format!("chartEx duplicate {tag}"))); }
    Ok(value)
}

fn cache(dimension: Node<'_, '_>) -> Result<Vec<String>> {
    cache_level(required(dimension, "lvl")?)
}

fn cache_level(level: Node<'_, '_>) -> Result<Vec<String>> {
    let count: usize = level.attribute("ptCount").and_then(|value| value.parse().ok()).ok_or_else(|| Error::Unsupported("chartEx cache count".into()))?;
    if !(1..=4096).contains(&count) { return Err(Error::Unsupported("chartEx cache requires 1..4096 points".into())); }
    let mut values = vec![None; count];
    for point in level.children().filter(|node| node.is_element()) {
        let index: usize = point.attribute("idx").and_then(|value| value.parse().ok()).ok_or_else(|| Error::Unsupported("chartEx cache index".into()))?;
        if !point.has_tag_name((NS, "pt")) || point.children().any(|node| node.is_element()) || index >= count || values[index].is_some() { return Err(Error::Unsupported("chartEx cache point".into())); }
        values[index] = Some(point.text().unwrap_or("").to_owned());
    }
    values.into_iter().map(|value| value.ok_or_else(|| Error::Unsupported("chartEx sparse cache".into()))).collect()
}

pub(crate) fn read(document: &roxmltree::Document<'_>, read_color: fn(Node<'_, '_>, &str) -> String) -> Result<(ChartKind, Vec<String>, Vec<ChartSeries>, ChartOptions)> {
    let root = document.root_element();
    let chart = required(root, "chart")?;
    let plot = required(chart, "plotArea")?;
    let region = required(plot, "plotAreaRegion")?;
    let entry = required(region, "series")?;
    let kind = match entry.attribute("layoutId") { Some("funnel") => ChartKind::Funnel, Some("waterfall") => ChartKind::Waterfall, Some("clusteredColumn") => ChartKind::Histogram, Some("boxWhisker") => ChartKind::BoxWhisker, Some("treemap") => ChartKind::Treemap, Some("sunburst") => ChartKind::Sunburst, _ => return Err(Error::Unsupported("unimplemented chartEx layout".into())) };
    if matches!(entry.attribute("hidden"), Some(value) if value != "0" && value != "false") || entry.attribute("ownerIdx").is_some() { return Err(Error::Unsupported("hidden/dependent chartEx series".into())); }
    let allowed_layout: &[&str] = match kind { ChartKind::Waterfall => &["subtotals"], ChartKind::Histogram => &["binning"], ChartKind::BoxWhisker => &["visibility", "statistics"], ChartKind::Treemap => &["parentLabelLayout"], _ => &[] };
    if child(entry, NS, "dataLabels").is_some() || child(chart, NS, "title").is_some() || child(entry, NS, "layoutPr").is_some_and(|node| node.children().any(|child| child.is_element() && !allowed_layout.iter().any(|tag| child.has_tag_name((NS, *tag))) && !child.has_tag_name((NS, "extLst")))) {
        return Err(Error::Unsupported("chartEx unrepresented labels/title/layout options".into()));
    }
    let axes: Vec<_> = plot.children().filter(|node| node.has_tag_name((NS, "axis"))).collect();
    let mut references = entry.children().filter(|node| node.has_tag_name((NS, "axisId"))).map(|node| node.text().and_then(|value| value.parse::<u32>().ok()).ok_or_else(|| Error::Unsupported("chartEx axis reference".into()))).collect::<Result<Vec<_>>>()?;
    if matches!(kind, ChartKind::Funnel | ChartKind::Treemap | ChartKind::Sunburst) {
        if !axes.is_empty() || !references.is_empty() { return Err(Error::Unsupported("funnel axes".into())); }
    } else {
        if references.is_empty() {
            for role in ["catScaling", "valScaling"] {
                let axis = axes.iter().find(|axis| child(**axis, NS, role).is_some()).ok_or_else(|| Error::Unsupported("waterfall axis role missing".into()))?;
                references.push(axis.attribute("id").and_then(|value| value.parse().ok()).ok_or_else(|| Error::Unsupported("chartEx axis identity".into()))?);
            }
        }
        if axes.len() != 2 || references.len() != 2 || references[0] == references[1] { return Err(Error::Unsupported("waterfall requires two distinct axes".into())); }
        let mut axis_ids = BTreeSet::new();
        for axis in axes {
            let id = axis.attribute("id").and_then(|value| value.parse::<u32>().ok()).ok_or_else(|| Error::Unsupported("chartEx axis identity".into()))?;
            if !axis_ids.insert(id) || !references.contains(&id) || matches!(axis.attribute("hidden"), Some(value) if value != "0" && value != "false") { return Err(Error::Unsupported("chartEx axis identity/visibility".into())); }
            let scaling = required(axis, if id == references[0] { "catScaling" } else { "valScaling" })?;
            if scaling.attributes().len() != 0 || scaling.children().any(|node| node.is_element()) || axis.children().any(|node| node.is_element() && !["catScaling", "valScaling", "majorGridlines", "minorGridlines", "tickLabels", "spPr", "txPr", "extLst"].iter().any(|tag| node.has_tag_name((NS, *tag)))) {
                return Err(Error::Unsupported("chartEx custom axis options".into()));
            }
        }
    }
    let data_id = required(entry, "dataId")?.attribute("val").and_then(|value| value.parse::<u32>().ok()).ok_or_else(|| Error::Unsupported("chartEx data reference".into()))?;
    let chart_data = required(root, "chartData")?;
    let mut ids = BTreeSet::new();
    let mut selected = None;
    for data in chart_data.children().filter(|node| node.has_tag_name((NS, "data"))) {
        let id = data.attribute("id").and_then(|value| value.parse::<u32>().ok()).ok_or_else(|| Error::Unsupported("chartEx data identity".into()))?;
        if !ids.insert(id) || ids.len() > 6 { return Err(Error::Unsupported("chartEx duplicate/oversized data blocks".into())); }
        if id == data_id { selected = Some(data); }
    }
    let data = selected.ok_or_else(|| Error::Unsupported("chartEx unresolved data identity".into()))?;
    let numbers = required(data, "numDim")?;
    if numbers.attribute("type") != Some(if matches!(kind, ChartKind::Treemap | ChartKind::Sunburst) { "size" } else { "val" }) { return Err(Error::Unsupported("chartEx numeric dimension role".into())); }
    let levels = if kind == ChartKind::Histogram {
        if child(data, NS, "strDim").is_some() { return Err(Error::Unsupported("histogram cannot have a category dimension".into())); }
        Vec::new()
    } else {
        let strings = required(data, "strDim")?;
        if strings.attribute("type") != Some("cat") { return Err(Error::Unsupported("chartEx category dimension role".into())); }
        let levels = strings.children().filter(|node| node.has_tag_name((NS, "lvl"))).map(cache_level).collect::<Result<Vec<_>>>()?;
        if levels.is_empty() || levels.len() > 4 || levels.iter().any(|level| level.len() != levels[0].len()) || (!matches!(kind, ChartKind::Treemap | ChartKind::Sunburst) && levels.len() != 1) { return Err(Error::Unsupported("chartEx category levels".into())); }
        levels
    };
    let mut categories = levels.first().cloned().unwrap_or_default();
    let mut values = cache(numbers)?.into_iter().map(|value| value.parse::<f64>().map_err(|_| Error::Unsupported("chartEx nonnumeric cache".into()))).collect::<Result<Vec<_>>>()?;
    if kind != ChartKind::Histogram && values.len() != categories.len() { return Err(Error::Unsupported("chartEx category/value cache length mismatch".into())); }
    let name = required(required(entry, "tx")?, "txData")?;
    let name = required(name, "v")?.text().unwrap_or("").to_owned();
    let shade = child(entry, NS, "spPr").map_or_else(|| "@accent1".into(), |node| read_color(node, "@accent1"));
    let legend = match child(chart, NS, "legend") {
        None => Some(LegendPosition::Hidden),
        Some(node) => match (node.attribute("pos").unwrap_or("r"), node.attribute("align").unwrap_or("ctr")) {
            ("b", "ctr") => None, ("t", "ctr") => Some(LegendPosition::Top), ("l", "ctr") => Some(LegendPosition::Left),
            ("r", "ctr") => Some(LegendPosition::Right), ("r", "max") => Some(LegendPosition::TopRight),
            _ => return Err(Error::Unsupported("chartEx legend alignment".into())),
        },
    };
    let totals = child(entry, NS, "layoutPr").and_then(|node| child(node, NS, "subtotals"));
    let waterfall_totals = if kind == ChartKind::Waterfall {
        Some(totals.map(|node| node.children().filter(|node| node.is_element()).map(|node| {
            if !node.has_tag_name((NS, "idx")) { return Err(Error::Unsupported("chartEx subtotal child".into())); }
            node.attribute("val").and_then(|value| value.parse().ok()).ok_or_else(|| Error::Unsupported("chartEx subtotal index".into()))
        }).collect::<Result<Vec<u32>>>()).transpose()?.unwrap_or_default())
    } else if totals.is_some() { return Err(Error::Unsupported("funnel subtotals".into())); } else { None };
    let mut options = ChartOptions { legend, waterfall_totals, ..Default::default() };
    if kind == ChartKind::Histogram {
        let binning = required(required(entry, "layoutPr")?, "binning")?;
        let bin_value = |node: Node<'_, '_>| -> Result<String> {
            if node.children().any(|child| child.is_element()) || node.attributes().any(|attribute| attribute.namespace().is_some() || attribute.name() != "val") { return Err(Error::Unsupported("unrepresented histogram bin metadata".into())); }
            let content = node.children().filter(|child| child.is_text()).filter_map(|child| child.text()).collect::<String>();
            match (node.attribute("val"), content.trim()) {
                (Some(value), "") => Ok(value.into()),
                (None, value) if !value.is_empty() => Ok(value.into()),
                _ => Err(Error::Unsupported("ambiguous histogram bin encoding".into())),
            }
        };
        let rule = match (child(binning, NS, "binCount"), child(binning, NS, "binSize")) {
            (Some(count), None) => HistogramBinning::Count { count: bin_value(count)?.parse().map_err(|_| Error::Unsupported("chartEx bin count".into()))? },
            (None, Some(width)) => HistogramBinning::Width { width: bin_value(width)?.parse().map_err(|_| Error::Unsupported("chartEx bin width".into()))? },
            _ => return Err(Error::Unsupported("histogram requires explicit bin count or width".into())),
        };
        let threshold = |name: &str| -> Result<Option<f64>> { binning.attribute(name).map(|value| value.parse().map_err(|_| Error::Unsupported("chartEx automatic/invalid threshold".into()))).transpose() };
        options.histogram = Some(HistogramOptions { samples: std::mem::take(&mut values), binning: rule, interval_closed: match binning.attribute("intervalClosed") { Some("l") => IntervalClosed::Left, Some("r") => IntervalClosed::Right, _ => return Err(Error::Unsupported("chartEx interval closure".into())) }, underflow: threshold("underflow")?, overflow: threshold("overflow")? });
    }
    if kind == ChartKind::BoxWhisker {
        let layout = required(entry, "layoutPr")?; let statistics = required(layout, "statistics")?; let visibility = required(layout, "visibility")?;
        let flag = |name: &str| -> Result<bool> { match visibility.attribute(name) { Some("1" | "true") => Ok(true), Some("0" | "false") => Ok(false), _ => Err(Error::Unsupported("chartEx statistical visibility".into())) } };
        let mut labels = Vec::new(); let mut samples: Vec<Vec<f64>> = Vec::new();
        for (label, value) in categories.into_iter().zip(std::mem::take(&mut values)) {
            if labels.last() != Some(&label) { labels.push(label); samples.push(Vec::new()); }
            samples.last_mut().ok_or_else(|| Error::Unsupported("chartEx sample group".into()))?.push(value);
        }
        categories = labels;
        options.box_whisker = Some(BoxWhiskerOptions { samples, quartile_method: match statistics.attribute("quartileMethod") { Some("inclusive") => QuartileMethod::Inclusive, Some("exclusive") => QuartileMethod::Exclusive, _ => return Err(Error::Unsupported("chartEx quartile method".into())) }, mean_line: flag("meanLine")?, mean_marker: flag("meanMarker")?, nonoutliers: flag("nonoutliers")?, outliers: flag("outliers")? });
    }
    if matches!(kind, ChartKind::Treemap | ChartKind::Sunburst) {
        let parent_labels = child(entry, NS, "layoutPr").and_then(|layout| child(layout, NS, "parentLabelLayout")).map(|node| match node.attribute("val") { Some("none") => Ok(ParentLabelLayout::None), Some("banner") => Ok(ParentLabelLayout::Banner), Some("overlapping") => Ok(ParentLabelLayout::Overlapping), _ => Err(Error::Unsupported("chartEx parent labels".into())) }).transpose()?;
        options.hierarchy = Some(HierarchyOptions { paths: (0..categories.len()).map(|index| levels.iter().rev().map(|level| level[index].clone()).collect()).collect(), parent_labels });
    }
    let series = vec![ChartSeries { name, values, color: shade, ..Default::default() }];
    crate::model::chart_format::validate(kind, &categories, &series, &options)?;
    Ok((kind, categories, series, options))
}

pub(crate) fn reference(writer: &mut XmlWriter, relationship: &str) {
    writer.start_element("a:graphicData"); writer.write_attribute("uri", NS);
    empty(writer, "cx:chart", &[("xmlns:cx", NS), ("r:id", relationship)]);
    writer.end_element();
}
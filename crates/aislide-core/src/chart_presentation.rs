use super::*;
use nalgebra::{DMatrix, DVector};

#[derive(Clone, Debug, Serialize)]
pub struct PresentationTick { pub value: f64, pub position: f64, pub label: String }

#[derive(Clone, Debug, Serialize)]
pub struct PresentationAxis {
    pub domain: [f64; 2],
    pub log_base: Option<f64>,
    pub reverse: bool,
    pub ticks: Vec<PresentationTick>,
    pub minor_ticks: Vec<PresentationTick>,
}

impl PresentationAxis {
    pub fn project(&self, value: f64) -> Result<f64> {
        if !bounded(value) || self.log_base.is_some() && value <= 0.0 {
            return Err(invalid("chart projection requires finite bounded values, positive on log axes"));
        }
        let map = |value: f64| if self.log_base.is_some() { value.ln() } else { value };
        let position = (map(value) - map(self.domain[0])) / (map(self.domain[1]) - map(self.domain[0]));
        if !position.is_finite() || position.abs() > 1e6 { return Err(invalid("chart projection exceeds coordinate budget")); }
        Ok(if self.reverse { 1.0 - position } else { position })
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct PresentationPoint { pub x: f64, pub y: f64 }

#[derive(Clone, Debug, Serialize)]
pub struct PresentationError {
    pub index: usize,
    pub direction: ErrorBarDirection,
    pub lower: PresentationPoint,
    pub upper: PresentationPoint,
}

#[derive(Clone, Debug, Serialize)]
pub struct PresentationTrend {
    pub points: Vec<PresentationPoint>,
    pub coefficients: Vec<f64>,
    pub center: f64,
    pub scale: f64,
    pub equation: Option<String>,
    pub r_squared: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PresentationSeries {
    pub axis: ChartAxis,
    pub points: Vec<PresentationPoint>,
    pub labels: Vec<String>,
    pub errors: Vec<PresentationError>,
    pub trend: Option<PresentationTrend>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChartPresentation {
    pub category_axis: PresentationAxis,
    pub primary_axis: PresentationAxis,
    pub secondary_axis: Option<PresentationAxis>,
    pub series: Vec<PresentationSeries>,
    pub warnings: Vec<String>,
    pub office_parity_verified: bool,
}

pub fn format_chart_number(value: f64, format: Option<&str>) -> (String, bool) {
    let general = || {
        if value == 0.0 { return "0".into(); }
        let exponent = value.abs().log10().floor() as i32;
        if !(-6..12).contains(&exponent) { return format!("{value:.6e}"); }
        let digits = (11 - exponent).clamp(0, 12) as usize;
        let text = format!("{value:.digits$}");
        if text.contains('.') { text.trim_end_matches('0').trim_end_matches('.').into() } else { text }
    };
    let format = format.unwrap_or("General");
    if format.eq_ignore_ascii_case("general") { return (general(), true); }
    let percent = format.ends_with('%');
    let pattern = format.strip_suffix('%').unwrap_or(format);
    let scientific = pattern.ends_with("E+00");
    let pattern = pattern.strip_suffix("E+00").unwrap_or(pattern);
    let (integer, fraction) = pattern.split_once('.').unwrap_or((pattern, ""));
    if !matches!(integer, "0" | "#,##0") || fraction.len() > 12 || !fraction.chars().all(|character| character == '0') || pattern.ends_with('.') {
        return (general(), false);
    }
    let magnitude = if percent { value * 100.0 } else { value };
    let mut text = if scientific {
        let text = format!("{:.*e}", fraction.len(), magnitude);
        let (mantissa, exponent) = text.split_once('e').unwrap_or((&text, "0"));
        format!("{mantissa}E{:+03}", exponent.parse::<i32>().unwrap_or(0))
    } else { format!("{:.*}", fraction.len(), magnitude) };
    if integer == "#,##0" && !scientific {
        let (whole, decimal) = text.split_once('.').unwrap_or((&text, ""));
        let negative = whole.starts_with('-');
        let digits = whole.trim_start_matches('-');
        let mut grouped = String::new();
        for (index, character) in digits.chars().enumerate() {
            if index > 0 && (digits.len() - index).is_multiple_of(3) { grouped.push(','); }
            grouped.push(character);
        }
        text = format!("{}{}{}{}", if negative { "-" } else { "" }, grouped, if decimal.is_empty() { "" } else { "." }, decimal);
    }
    if percent { text.push('%'); }
    (text, true)
}

fn axis(options: &AxisOptions, values: &[f64], zero: bool, percent: bool, warnings: &mut Vec<String>) -> Result<PresentationAxis> {
    if values.iter().any(|value| !bounded(*value) || options.log_base.is_some() && *value <= 0.0) {
        return Err(invalid("derived chart values must be finite in +/-1e15 and positive for logarithmic axes"));
    }
    let mut low = values.iter().copied().reduce(f64::min).unwrap_or(0.0);
    let mut high = values.iter().copied().reduce(f64::max).unwrap_or(1.0);
    if zero && options.log_base.is_none() { low = low.min(0.0); high = high.max(0.0); }
    if percent { low = 0.0; high = 1.0; }
    if let Some(base) = options.log_base {
        low = base.powf((low.ln() / base.ln()).floor());
        high = base.powf((high.ln() / base.ln()).ceil());
    }
    low = options.min.unwrap_or(low);
    high = options.max.unwrap_or(high);
    if high <= low {
        let delta = low.abs().max(1.0) * 0.1;
        if options.max.is_some() && options.min.is_none() { low = options.log_base.map_or(high - delta, |base| high / base); }
        else if options.max.is_none() { high = options.log_base.map_or(low + delta, |base| low * base); }
    }
    if !low.is_finite() || !high.is_finite() || low >= high || options.log_base.is_some() && low <= 0.0 { return Err(invalid("presentation axis domain must increase")); }
    let mut output = PresentationAxis { domain: [low, high], log_base: options.log_base, reverse: options.reverse, ticks: vec![], minor_ticks: vec![] };
    let default_format = if percent { "0%" } else { "General" };
    let format = options.number_format.as_deref().unwrap_or(default_format);
    if !format_chart_number(1.0, Some(format)).1 { warnings.push(format!("Unsupported number format {format:?}; General used for preview only")); }
    for minor in [false, true] {
        let values = if let Some(base) = options.log_base {
            if minor { vec![] } else {
                let first = (low.ln() / base.ln() - 1e-10).ceil();
                let count = (high.ln() / base.ln() + 1e-10).floor() - first + 1.0;
                if count > 128.0 { return Err(Error::Limit("chart ticks > 128 per axis".into())); }
                (0..count.max(0.0) as usize).map(|index| base.powf(first + index as f64)).collect()
            }
        } else if let Some(unit) = if minor { options.minor_unit } else { options.major_unit } {
            let first = (low / unit).ceil() * unit;
            let count = ((high - first) / unit + 1e-10).floor() + 1.0;
            if !count.is_finite() || count > 128.0 { return Err(Error::Limit("chart ticks > 128 per axis".into())); }
            (0..count.max(0.0) as usize).map(|index| first + index as f64 * unit).collect()
        } else if minor { vec![] } else {
            let raw = (high - low) / 5.0;
            let power = 10.0f64.powf(raw.log10().floor());
            let fraction = raw / power;
            let unit = power * if fraction <= 1.0 { 1.0 } else if fraction <= 2.0 { 2.0 } else if fraction <= 5.0 { 5.0 } else { 10.0 };
            let first = (low / unit).ceil() * unit;
            let count = ((high - first) / unit + 1e-10).floor().max(0.0) as usize + 1;
            (0..count.min(128)).map(|index| first + index as f64 * unit).collect()
        };
        let ticks = values.into_iter().map(|value| Ok(PresentationTick { value, position: output.project(value)?, label: format_chart_number(value, Some(format)).0 })).collect::<Result<Vec<_>>>()?;
        if minor { output.minor_ticks = ticks; } else { output.ticks = ticks; }
    }
    Ok(output)
}

fn trend(coordinates: &[f64], values: &[f64], config: &Trendline) -> Result<PresentationTrend> {
    if config.kind == TrendlineKind::MovingAverage {
        let period = usize::from(config.period.ok_or_else(|| invalid("moving average period missing"))?);
        return Ok(PresentationTrend { points: values.windows(period).enumerate().map(|(index, window)| PresentationPoint { x: coordinates[index + period - 1], y: window.iter().sum::<f64>() / period as f64 }).collect(), coefficients: vec![], center: 0.0, scale: 1.0, equation: None, r_squared: None });
    }
    let log_x = matches!(config.kind, TrendlineKind::Logarithmic | TrendlineKind::Power);
    let log_y = matches!(config.kind, TrendlineKind::Exponential | TrendlineKind::Power);
    if log_x && coordinates.iter().any(|value| *value <= 0.0) || log_y && values.iter().any(|value| *value <= 0.0) {
        return Err(invalid("logarithmic regression requires strictly positive transformed observations"));
    }
    if log_y && config.intercept.is_some_and(|value| value <= 0.0) { return Err(invalid("fixed exponential intercept must be positive (Y at X=0)")); }
    let transformed_x: Vec<_> = coordinates.iter().map(|value| if log_x { value.ln() } else { *value }).collect();
    let transformed_y: Vec<_> = values.iter().map(|value| if log_y { value.ln() } else { *value }).collect();
    let degree = usize::from(config.order.unwrap_or(1));
    let fixed = config.intercept.map(|value| if log_y { value.ln() } else { value });
    let center = if fixed.is_some() { 0.0 } else { transformed_x.iter().sum::<f64>() / transformed_x.len() as f64 };
    let scale = transformed_x.iter().map(|value| (value - center).abs()).fold(0.0, f64::max);
    if scale <= f64::EPSILON * center.abs().max(1.0) * 32.0 { return Err(invalid("rank-deficient trendline X range")); }
    let first_power = usize::from(fixed.is_some());
    let columns = degree + 1 - first_power;
    let matrix = DMatrix::from_fn(coordinates.len(), columns, |row, column| ((transformed_x[row] - center) / scale).powi((column + first_power) as i32));
    let target = DVector::from_iterator(values.len(), transformed_y.iter().map(|value| value - fixed.unwrap_or(0.0)));
    let svd = matrix.svd(true, true);
    let tolerance = svd.singular_values.iter().copied().fold(0.0, f64::max) * 1e-12;
    if svd.singular_values.iter().filter(|value| **value > tolerance).count() != columns { return Err(invalid("rank-deficient trendline design matrix")); }
    let solved = svd.solve(&target, tolerance).map_err(|_| invalid("trendline SVD failed"))?;
    let mut coefficients = Vec::new();
    if let Some(intercept) = fixed { coefficients.push(intercept); }
    coefficients.extend(solved.iter().copied());
    let evaluate = |coordinate: f64| -> Result<f64> {
        if log_x && coordinate <= 0.0 { return Err(invalid("forecast extends outside positive X domain")); }
        let normalized = ((if log_x { coordinate.ln() } else { coordinate }) - center) / scale;
        let fitted = coefficients.iter().rev().fold(0.0, |sum, coefficient| sum * normalized + coefficient);
        let fitted = if log_y { fitted.exp() } else { fitted };
        if !bounded(fitted) { return Err(invalid("trendline result exceeds +/-1e15")); }
        Ok(fitted)
    };
    let minimum = coordinates.iter().copied().reduce(f64::min).ok_or_else(|| invalid("empty trendline"))?;
    let maximum = coordinates.iter().copied().reduce(f64::max).ok_or_else(|| invalid("empty trendline"))?;
    let backward = config.backward.unwrap_or(0.0);
    let forward = config.forward.unwrap_or(0.0);
    if backward > 100_000.0 || forward > 100_000.0 { return Err(Error::Limit("trendline forecast > 100000 X units".into())); }
    let low = minimum - backward;
    let high = maximum + forward;
    if !bounded(low) || !bounded(high) { return Err(invalid("forecast X exceeds +/-1e15")); }
    let mut samples: Vec<_> = (0..224).map(|index| low + (high - low) * index as f64 / 223.0).chain(coordinates.iter().copied()).collect();
    samples.sort_by(f64::total_cmp);
    samples.dedup();
    let points = samples.into_iter().map(|x| Ok(PresentationPoint { x, y: evaluate(x)? })).collect::<Result<Vec<_>>>()?;
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let total = values.iter().map(|value| (value - mean).powi(2)).sum::<f64>();
    let residual = coordinates.iter().zip(values).map(|(coordinate, value)| Ok((value - evaluate(*coordinate)?).powi(2))).collect::<Result<Vec<_>>>()?.iter().sum::<f64>();
    let r_squared = if total > f64::EPSILON * values.iter().map(|value| value * value).sum::<f64>() { Some(1.0 - residual / total) } else { None };
    let equation = config.display_equation.then(|| {
        let expression = coefficients.iter().enumerate().map(|(power, value)| format!("{value:.6}*z^{power}")).collect::<Vec<_>>().join(" + ");
        format!("{} = {expression}; z = ({} - {center:.6})/{scale:.6}", if log_y { "ln(y)" } else { "y" }, if log_x { "ln(x)" } else { "x" })
    });
    Ok(PresentationTrend { points, coefficients, center, scale, equation, r_squared })
}

pub fn compute_chart_presentation(kind: ChartKind, categories: &[String], series: &[ChartSeries], options: &ChartOptions) -> Result<ChartPresentation> {
    validate(kind, categories, series, options)?;
    if kind.is_extended() || kind.is_polar() || matches!(kind, ChartKind::Radar | ChartKind::RadarFilled) { return Err(Error::Unsupported("cartesian presentation required".into())); }
    let coordinates: Vec<_> = categories.iter().enumerate().map(|(index, value)| if kind.is_xy() { value.parse::<f64>().map_err(|_| invalid("invalid X coordinate")) } else { Ok(index as f64 + 1.0) }).collect::<Result<_>>()?;
    let mut warnings = vec!["AISlide bounded chart projection; not Office visual parity".into()];
    let mut projected = Vec::new();
    for (series_index, entry) in series.iter().enumerate() {
        let points = entry.values.iter().enumerate().map(|(index, value)| PresentationPoint { x: coordinates[index], y: if kind.is_percent() { value / series.iter().map(|entry| entry.values[index]).sum::<f64>() } else { *value } }).collect();
        let mut errors = Vec::new();
        if let Some(config) = &entry.error_bars {
            let observations = if config.direction == ErrorBarDirection::X { &coordinates } else { &entry.values };
            let mean = observations.iter().sum::<f64>() / observations.len() as f64;
            let deviation = (observations.iter().map(|value| (value - mean).powi(2)).sum::<f64>() / (observations.len() - 1).max(1) as f64).sqrt();
            for (index, observation) in observations.iter().enumerate() {
                let center = if config.kind == ErrorBarKind::StandardDeviation { mean } else { *observation };
                let amount = match config.kind { ErrorBarKind::FixedValue => config.value.unwrap_or(0.0), ErrorBarKind::Percentage => observation.abs() * config.value.unwrap_or(0.0) / 100.0, ErrorBarKind::StandardDeviation => deviation * config.value.unwrap_or(0.0), ErrorBarKind::StandardError => deviation / (observations.len() as f64).sqrt(), ErrorBarKind::Custom => 0.0 };
                let lower = center - if config.bar_type == ErrorBarType::Plus { 0.0 } else { config.minus.as_ref().map_or(amount, |values| values[index]) };
                let upper = center + if config.bar_type == ErrorBarType::Minus { 0.0 } else { config.plus.as_ref().map_or(amount, |values| values[index]) };
                let endpoint = |value| if config.direction == ErrorBarDirection::X { PresentationPoint { x: value, y: entry.values[index] } } else { PresentationPoint { x: coordinates[index], y: value } };
                errors.push(PresentationError { index, direction: config.direction, lower: endpoint(lower), upper: endpoint(upper) });
            }
        }
        let trend = entry.trendline.as_ref().map(|config| trend(&coordinates, &entry.values, config)).transpose()?;
        if entry.trendline.as_ref().is_some_and(|config| config.display_r_squared) && trend.as_ref().is_some_and(|trend| trend.r_squared.is_none()) { warnings.push(format!("{}: R squared undefined for constant response", entry.name)); }
        let labels = entry.values.iter().enumerate().map(|(index, value)| {
            let Some(labels) = &options.data_labels else { return String::new(); };
            let mut parts = Vec::new();
            if labels.show_series_name { parts.push(series[series_index].name.clone()); }
            if labels.show_category_name { parts.push(categories[index].clone()); }
            if labels.show_value { parts.push(format_chart_number(*value, labels.number_format.as_deref()).0); }
            parts.join(" ")
        }).collect();
        projected.push(PresentationSeries { axis: entry.axis.unwrap_or_default(), points, labels, errors, trend });
    }
    if options.data_labels.as_ref().and_then(|labels| labels.number_format.as_deref()).is_some_and(|format| !format_chart_number(1.0, Some(format)).1) { warnings.push("Unsupported data-label format; General used for preview only".into()); }
    let all_points = |axis: Option<ChartAxis>| projected.iter().filter(move |entry| axis.is_none_or(|axis| axis == entry.axis)).flat_map(|entry| entry.points.iter().chain(entry.errors.iter().flat_map(|error| [&error.lower, &error.upper])).chain(entry.trend.iter().flat_map(|trend| &trend.points)));
    let mut x_values: Vec<_> = all_points(None).map(|point| point.x).collect();
    if !kind.is_xy() { x_values.extend([0.5, categories.len() as f64 + 0.5]); }
    let mut category_axis = axis(&options.category_axis, &x_values, false, false, &mut warnings)?;
    if !kind.is_xy() {
        category_axis.ticks = categories.iter().enumerate().map(|(index, label)| {
            let value = index as f64 + 1.0;
            Ok(PresentationTick { value, position: category_axis.project(value)?, label: if let (Ok(number), Some(format)) = (label.parse::<f64>(), options.category_axis.number_format.as_deref()) { format_chart_number(number, Some(format)).0 } else { label.clone() } })
        }).collect::<Result<_>>()?;
    }
    let stacked = matches!(kind, ChartKind::StackedColumn | ChartKind::StackedBar) || kind.is_percent();
    let mut value_axis = |selected, config: &AxisOptions| {
        let mut values: Vec<_> = all_points(Some(selected)).map(|point| point.y).collect();
        if stacked { values.extend((0..categories.len()).flat_map(|index| [series.iter().map(|entry| entry.values[index].min(0.0)).sum::<f64>(), series.iter().map(|entry| entry.values[index].max(0.0)).sum::<f64>()])); }
        let zero = series.iter().any(|entry| entry.axis.unwrap_or_default() == selected && matches!(entry.kind.unwrap_or(kind), ChartKind::Column | ChartKind::Bar | ChartKind::Column3d | ChartKind::Bar3d | ChartKind::Area | ChartKind::StackedColumn | ChartKind::StackedBar));
        axis(config, &values, zero, kind.is_percent(), &mut warnings)
    };
    let primary_axis = value_axis(ChartAxis::Primary, &options.primary_axis)?;
    let secondary_axis = if series.iter().any(|entry| entry.axis == Some(ChartAxis::Secondary)) { Some(value_axis(ChartAxis::Secondary, &options.secondary_axis)?) } else { None };
    for entry in &projected {
        let value_axis = if entry.axis == ChartAxis::Secondary { secondary_axis.as_ref().unwrap_or(&primary_axis) } else { &primary_axis };
        for point in entry.points.iter().chain(entry.errors.iter().flat_map(|error| [&error.lower, &error.upper])).chain(entry.trend.iter().flat_map(|trend| &trend.points)) { category_axis.project(point.x)?; value_axis.project(point.y)?; }
    }
    Ok(ChartPresentation { category_axis, primary_axis, secondary_axis, series: projected, warnings, office_parity_verified: false })
}
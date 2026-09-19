use super::{Drawing, PartData, accent, display};
use crate::{model::{ChartKind, ChartSeries, TextAlign}, Error, Result};

pub(super) fn render(drawing: &mut Drawing, category: &str, variant: usize, data: &PartData) -> Result<()> {
    if category == "infographic" { return infographic(drawing, variant, data); }
    if category == "water-fall" { return waterfall(drawing, variant, data); }
    let PartData::Chart { categories, series, x_axis, y_axis } = data else { return Err(Error::Invalid("chart preset requires chart data".into())); };
    let kind = match category {
        "pie-chart" => if variant == 1 { ChartKind::Pie } else { ChartKind::Doughnut },
        "vertical-bar-graph" => ChartKind::Column, "horizontal-bar-graph" => ChartKind::Bar,
        "add-vertical-bar-graph" => ChartKind::StackedColumn,
        "100-add-vertical-bar-graph" => ChartKind::PercentStackedColumn,
        "area-graph" => ChartKind::Area, "line-graph" => ChartKind::Line, "scatter" => ChartKind::Scatter,
        _ => return Err(Error::Unsupported("chart category".into())),
    };
    let series: Vec<_> = series.iter().enumerate().map(|(index, entry)| ChartSeries { name:entry.name.clone(),values:entry.values.clone(),color:accent(index), ..Default::default() }).collect();
    crate::model::validate_chart_kind(kind, categories, &series)?;
    if kind == ChartKind::Area && series.len() > 1 { return Err(Error::Invalid("area overview currently requires a single series to avoid occluded data".into())); }
    let chart_bounds = match variant { 1 => [48.0,108.0,740.0,344.0], 2 => [48.0,100.0,720.0,360.0], _ => [64.0,104.0,1024.0,348.0] };
    drawing.chart(kind, categories.clone(), series.clone(), chart_bounds);
    if !kind.is_polar() {
        drawing.text(x_axis,[chart_bounds[0],470.0,chart_bounds[2],28.0],16.0,"@dk2",false,TextAlign::Center);
        drawing.text(if kind==ChartKind::PercentStackedColumn { "Share (%)" } else {y_axis},[chart_bounds[0],76.0,chart_bounds[2],24.0],14.0,"@dk2",false,TextAlign::Left);
    }
    if variant == 1 {
        let value = series[0].values.last().copied().unwrap_or(0.0);
        let value = if kind == ChartKind::PercentStackedColumn {format!("{}%",display(value / series.iter().map(|entry|entry.values.last().copied().unwrap_or(0.0)).sum::<f64>() * 100.0))} else {display(value)};
        drawing.rect([824.0,120.0,3.0,304.0],"@accent1");
        drawing.text(&value,[856.0,172.0,264.0,74.0],52.0,"@accent1",true,TextAlign::Left);
        drawing.text(categories.last().map(String::as_str).unwrap_or(""),[856.0,252.0,264.0,52.0],24.0,"@dk1",true,TextAlign::Left);
        drawing.text(&series[0].name,[856.0,318.0,264.0,68.0],18.0,"@dk2",false,TextAlign::Left);
    } else if variant == 2 {
        let row_height = 340.0 / categories.len() as f64;
        for (index, category) in categories.iter().enumerate() {
            let top=104.0+index as f64*row_height;
            drawing.rect([816.0,top,304.0,1.0],"@lt2");
            drawing.text(category,[824.0,top+5.0,132.0,row_height-7.0],16.0,"@dk1",true,TextAlign::Left);
            let values=series.iter().map(|series| display(series.values[index])).collect::<Vec<_>>().join(" / ");
            drawing.text(&values,[966.0,top+5.0,154.0,row_height-7.0],14.0,"@dk2",false,TextAlign::Right);
        }
    }
    Ok(())
}

fn waterfall(drawing: &mut Drawing, variant: usize, data: &PartData) -> Result<()> {
    let PartData::Waterfall { steps, unit } = data else { return Err(Error::Invalid("waterfall requires steps with explicit totals".into())); };
    let mut running=0.0; let mut segments=Vec::new(); let mut minimum=0.0f64; let mut maximum=0.0f64;
    for step in steps { let from=if step.total {0.0} else {running}; let to=if step.total {step.value} else {running+step.value}; running=to; minimum=minimum.min(from).min(to); maximum=maximum.max(from).max(to); segments.push((from,to)); }
    let span=(maximum-minimum).max(1.0); let bottom=if variant==2 {400.0} else {440.0}; let height=bottom-140.0;
    let ordinate=|value:f64| bottom-(value-minimum)/span*height; let slot=1040.0/steps.len() as f64;
    drawing.line([64.0,ordinate(0.0)],[1104.0,ordinate(0.0)],false,"@dk2");
    drawing.text(unit,[64.0,82.0,400.0,26.0],16.0,"@dk2",false,TextAlign::Left);
    for (index,(step,(from,to))) in steps.iter().zip(segments.iter()).enumerate() {
        let left=64.0+index as f64*slot+slot*0.16; let width=slot*0.68; let upper=ordinate(from.max(*to)); let lower=ordinate(from.min(*to));
        let shade=if step.total {"@dk1"} else if step.value>=0.0 {"@accent1"} else {"@accent2"};
        drawing.rect([left,upper,width,(lower-upper).max(0.01)],shade);
        drawing.text(&display(step.value),[left-10.0,upper-30.0,width+20.0,26.0],if variant==1 && step.total {22.0} else {16.0},shade,true,TextAlign::Center);
        drawing.text(&step.label,[left-slot*0.1,bottom+12.0,slot*0.88,52.0],16.0,"@dk1",false,TextAlign::Center);
        if index+1<steps.len() { drawing.line([left+width,ordinate(*to)],[left+slot,ordinate(*to)],false,"@dk2"); }
        if variant==2 { drawing.text(&format!("= {}",display(*to)),[left-6.0,472.0,width+12.0,28.0],15.0,"@dk2",false,TextAlign::Center); }
    }
    Ok(())
}

fn infographic(drawing: &mut Drawing, variant: usize, data: &PartData) -> Result<()> {
    let PartData::Items { items, .. } = data else { return Err(Error::Invalid("pictogram requires labeled values".into())); };
    super::count(items.len(),2,4)?;
    if items.iter().any(|item| item.value.is_none_or(|value| !(0.0..=100.0).contains(&value))) { return Err(Error::Invalid("pictogram values must be percentages 0-100".into())); }
    let row_height=400.0/items.len() as f64;
    for (index,item) in items.iter().enumerate() {
        let value=item.value.unwrap_or(0.0); let top=96.0+index as f64*row_height; let shade=accent(index);
        drawing.text(&item.label,[24.0,top,232.0,48.0],20.0,"@dk1",true,TextAlign::Left);
        drawing.text(&format!("{}%",display(value)),[960.0,top,168.0,52.0],32.0,&shade,true,TextAlign::Right);
        if variant==2 { drawing.rect([280.0,top+12.0,640.0,26.0],"@lt2"); drawing.rect([280.0,top+12.0,640.0*value/100.0,26.0],&shade); }
        else {
            for dot in 0..20 { let filled=(value/5.0-dot as f64).clamp(0.0,1.0); let bounds=[280.0+dot as f64*32.0,top+12.0,22.0,22.0];
                drawing.shape(if variant==0 {"ellipse"} else {"rect"},bounds,if filled==1.0 {&shade} else {"@lt2"},"@lt2");
                if filled>0.0 && filled<1.0 { drawing.rect([bounds[0],bounds[1]+26.0,22.0*filled,3.0],&shade); }
            }
            drawing.text("1 unit = 5%; fractional unit shown below",[280.0,top+52.0,640.0,24.0],12.0,"@dk2",false,TextAlign::Left);
        }
    }
    Ok(())
}
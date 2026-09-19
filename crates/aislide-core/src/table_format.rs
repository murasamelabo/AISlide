use crate::{model::{Element, TextFormat, VerticalAlign, validate_rows, valid_color}, rich_text::RunStyle, Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DimensionUnit { Relative, Absolute }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dimensions { pub unit: DimensionUnit, pub values: Vec<f64> }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MergeRegion { pub row: usize, pub column: usize, pub row_span: usize, pub col_span: usize }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellPadding { pub left: f64, pub right: f64, pub top: f64, pub bottom: f64 }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellOutline { pub color: String, pub width: f64 }

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CellStyle {
    #[serde(skip_serializing_if = "Option::is_none")] pub fill: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub outline: Option<CellOutline>,
    #[serde(skip_serializing_if = "Option::is_none")] pub padding: Option<CellPadding>,
    #[serde(skip_serializing_if = "Option::is_none")] pub vertical: Option<VerticalAlign>,
    #[serde(skip_serializing_if = "Option::is_none")] pub text_style: Option<RunStyle>,
    #[serde(skip_serializing_if = "Option::is_none")] pub text_format: Option<TextFormat>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellFormat { pub row: usize, pub column: usize, pub style: CellStyle }

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TableFormat {
    #[serde(skip_serializing_if = "Option::is_none")] pub column_widths: Option<Dimensions>,
    #[serde(skip_serializing_if = "Option::is_none")] pub row_heights: Option<Dimensions>,
    #[serde(skip_serializing_if = "Vec::is_empty")] pub merges: Vec<MergeRegion>,
    #[serde(skip_serializing_if = "Vec::is_empty")] pub cells: Vec<CellFormat>,
}

impl TableFormat { pub fn is_default(&self) -> bool { self == &Self::default() } }

impl Dimensions {
    pub fn resolve(&self, count: usize, extent: f64) -> Result<Vec<f64>> {
        if count == 0 || count > 12 || self.values.len() != count || !extent.is_finite() || !(0.01..=4096.0).contains(&extent)
            || self.values.iter().any(|value| !value.is_finite() || !(0.000001..=1_000_000.0).contains(value)) {
            return Err(Error::Invalid("table dimensions require bounded positive values for every track".into()));
        }
        let sum: f64 = self.values.iter().sum();
        if self.unit == DimensionUnit::Absolute && (sum - extent).abs() > 0.001 { return Err(Error::Invalid("absolute table dimensions must sum to the frame extent".into())); }
        let values: Vec<_> = self.values.iter().map(|value| value * extent / sum).collect();
        if values.iter().any(|value| *value < 0.01) { return Err(Error::Invalid("table track is smaller than 0.01px".into())); }
        Ok(values)
    }
}

pub(crate) fn tracks(dimensions: Option<&Dimensions>, count: usize, extent: f64) -> Result<Vec<f64>> {
    dimensions.map_or_else(|| Dimensions { unit: DimensionUnit::Relative, values: vec![1.0; count] }.resolve(count, extent), |dimensions| dimensions.resolve(count, extent))
}

impl MergeRegion {
    fn contains(&self, row: usize, column: usize) -> bool {
        row >= self.row && row - self.row < self.row_span && column >= self.column && column - self.column < self.col_span
    }
}

impl TableFormat {
    pub fn validate(&self, rows: &[Vec<String>], width: f64, height: f64) -> Result<()> {
        validate_rows(rows)?;
        if rows.iter().flatten().any(|cell| cell.contains('\r')) { return Err(Error::Invalid("table text uses LF paragraph separators; CR would be normalized by XML".into())); }
        tracks(self.column_widths.as_ref(), rows[0].len(), width)?;
        tracks(self.row_heights.as_ref(), rows.len(), height)?;
        if self.merges.len() > 96 || self.cells.len() > 96 { return Err(Error::Limit("table formatting exceeds 96 cells or merges".into())); }
        let mut occupied = std::collections::BTreeSet::new();
        for region in &self.merges {
            if region.row_span == 0 || region.col_span == 0 || (region.row_span == 1 && region.col_span == 1)
                || region.row.checked_add(region.row_span).is_none_or(|end| end > rows.len())
                || region.column.checked_add(region.col_span).is_none_or(|end| end > rows[0].len()) {
                return Err(Error::Invalid("merged table region is outside the grid or has no span".into()));
            }
            for row in region.row..region.row + region.row_span {
                for column in region.column..region.column + region.col_span {
                    if !occupied.insert((row,column)) { return Err(Error::Invalid("table merges must be disjoint".into())); }
                    if (row,column) != (region.row,region.column) && !rows[row][column].is_empty() { return Err(Error::Invalid("merged subordinate cells must be empty; contents are never silently discarded".into())); }
                }
            }
        }
        let mut cells = std::collections::BTreeSet::new();
        for cell in &self.cells {
            if cell.row >= rows.len() || cell.column >= rows[0].len() || !cells.insert((cell.row,cell.column)) { return Err(Error::Invalid("cell style index outside grid or duplicated".into())); }
            let style = &cell.style;
            if let Some(fill) = &style.fill { if fill != "none" { valid_color(fill)?; } }
            if let Some(outline) = &style.outline {
                valid_color(&outline.color)?;
                if !outline.width.is_finite() || !(0.0..=20.0).contains(&outline.width) { return Err(Error::Invalid("table outline width must be 0..20px".into())); }
            }
            if let Some(padding) = &style.padding {
                if [padding.left,padding.right,padding.top,padding.bottom].iter().any(|value| !value.is_finite() || !(0.0..=4096.0).contains(value)) { return Err(Error::Invalid("table padding must be finite in 0..4096px".into())); }
            }
            if let Some(run) = &style.text_style { run.validate()?; }
            if let Some(format) = &style.text_format {
                if format.paragraphs.iter().flat_map(|paragraph| &paragraph.runs).any(|run| run.field.is_some()) { return Err(Error::Unsupported("dynamic fields in table cells are not supported".into())); }
                if format.hyperlink.is_some() || format.placeholder.is_some() || format.inherit_layout || format.vertical != VerticalAlign::Top { return Err(Error::Unsupported("table text does not support links, placeholders, inheritance or body vertical alignment; use cell vertical".into())); }
                RunStyle { font_family: format.font_family.clone(), ..Default::default() }.validate()?;
                crate::rich_text::validate_paragraphs(&format.paragraphs)?;
                if !format.paragraphs.is_empty() && crate::rich_text::plain_text(&format.paragraphs) != rows[cell.row][cell.column] { return Err(Error::Invalid("cell text must equal joined rich paragraph runs".into())); }
            }
        }
        Ok(())
    }

    pub fn cell_style(&self, row: usize, column: usize) -> CellStyle {
        self.cells.iter().find(|cell| cell.row == row && cell.column == column).map(|cell| cell.style.clone()).unwrap_or_default()
    }

    pub(crate) fn merge_at(&self, row: usize, column: usize) -> Option<&MergeRegion> { self.merges.iter().find(|region| region.contains(row,column)) }
}

pub fn validate_element(element: &Element) -> Result<()> {
    let Element::Table { rows, width, height, font_size, format, .. } = element else { return Err(Error::Unsupported("table operation requires a table".into())); };
    if !font_size.is_finite() || !(8.0..=120.0).contains(font_size) { return Err(Error::Invalid("table font size must be 8..120px".into())); }
    format.validate(rows,*width,*height)
}

pub fn update_format(mut element: Element, replacement: TableFormat) -> Result<Element> {
    validate_element(&element)?;
    if let Element::Table { format, .. } = &mut element { *format = replacement; }
    validate_element(&element)?; Ok(element)
}

pub fn replace_cell_text(mut element: Element, row: usize, column: usize, replacement: String) -> Result<Element> {
    validate_element(&element)?;
    crate::model::valid_text(&replacement, 200)?;
    let Element::Table { rows, format, font_size, .. } = &mut element else { unreachable!() };
    let text = rows.get_mut(row).and_then(|cells| cells.get_mut(column)).ok_or_else(|| Error::Invalid("cell index outside grid".into()))?;
    if *text == replacement { return Ok(element); }
    let rich = format.cells.iter_mut().find(|cell| (cell.row, cell.column) == (row, column))
        .and_then(|cell| cell.style.text_format.as_mut());
    if let Some(rich) = rich.filter(|rich| !rich.paragraphs.is_empty()) {
        let proxy = Element::Text { id: "cell".into(), x: 0.0, y: 0.0, width: 1.0, height: 1.0,
            text: text.clone(), font_size: *font_size, color: "000000".into(), bold: false, format: rich.clone(), visual: None };
        let Element::Text { text: updated_text, format: updated_format, .. } = crate::rich_text::replace_text_content(proxy, replacement)? else { unreachable!() };
        *text = updated_text;
        *rich = updated_format;
    } else { *text = replacement; }
    validate_element(&element)?;
    Ok(element)
}

pub fn set_cell_style(mut element: Element, row: usize, column: usize, style: CellStyle) -> Result<Element> {
    validate_element(&element)?;
    if let Element::Table { format, rows, .. } = &mut element {
        if row >= rows.len() || column >= rows[0].len() { return Err(Error::Invalid("cell index outside grid".into())); }
        format.cells.retain(|cell| (cell.row,cell.column) != (row,column));
        if style != CellStyle::default() { format.cells.push(CellFormat { row,column,style }); }
        format.cells.sort_by_key(|cell| (cell.row,cell.column));
    }
    validate_element(&element)?; Ok(element)
}

pub fn merge_cells(mut element: Element, region: MergeRegion) -> Result<Element> {
    validate_element(&element)?;
    if let Element::Table { format, .. } = &mut element { format.merges.push(region); format.merges.sort_by_key(|region| (region.row,region.column)); }
    validate_element(&element)?; Ok(element)
}

pub fn split_cell(mut element: Element, row: usize, column: usize) -> Result<Element> {
    validate_element(&element)?;
    if let Element::Table { format, rows, .. } = &mut element {
        if row >= rows.len() || column >= rows[0].len() { return Err(Error::Invalid("cell index outside grid".into())); }
        format.merges.retain(|region| !region.contains(row,column));
    }
    Ok(element)
}

pub fn insert_row(element: Element, index: usize, values: Vec<String>) -> Result<Element> { edit_axis(element,index,Some(values),true) }
pub fn insert_column(element: Element, index: usize, values: Vec<String>) -> Result<Element> { edit_axis(element,index,Some(values),false) }
pub fn remove_row(element: Element, index: usize) -> Result<Element> { edit_axis(element,index,None,true) }
pub fn remove_column(element: Element, index: usize) -> Result<Element> { edit_axis(element,index,None,false) }

fn edit_axis(mut element: Element, index: usize, values: Option<Vec<String>>, vertical: bool) -> Result<Element> {
    validate_element(&element)?;
    let Element::Table { rows, format, width, height, .. } = &mut element else { unreachable!() };
    let count = if vertical { rows.len() } else { rows[0].len() };
    let other = if vertical { rows[0].len() } else { rows.len() };
    let inserting = values.is_some();
    if index > count || (!inserting && (index == count || count == 1)) || (inserting && count == if vertical {12} else {8}) || values.as_ref().is_some_and(|values| values.len() != other) { return Err(Error::Invalid("invalid table axis edit".into())); }
    for region in &mut format.merges {
        let start = if vertical { &mut region.row } else { &mut region.column };
        let span = if vertical { region.row_span } else { region.col_span };
        if (inserting && index > *start && index < *start + span) || (!inserting && index >= *start && index < *start + span) { return Err(Error::Unsupported("axis edit crosses a merge; split the region first".into())); }
        if (inserting && index <= *start) || (!inserting && index < *start) { *start = if inserting {*start + 1} else {*start - 1}; }
    }
    format.cells.retain(|cell| inserting || if vertical {cell.row != index} else {cell.column != index});
    for cell in &mut format.cells {
        let coordinate = if vertical { &mut cell.row } else { &mut cell.column };
        if (inserting && *coordinate >= index) || (!inserting && *coordinate > index) { *coordinate = if inserting {*coordinate + 1} else {*coordinate - 1}; }
    }
    let dimensions = if vertical { &mut format.row_heights } else { &mut format.column_widths };
    if let Some(dimensions) = dimensions {
        let sum: f64 = dimensions.values.iter().sum();
        if inserting { dimensions.values.insert(index,sum / count as f64); } else { dimensions.values.remove(index); }
        if dimensions.unit == DimensionUnit::Absolute {
            let total: f64 = dimensions.values.iter().sum(); let extent = if vertical {*height} else {*width};
            for value in &mut dimensions.values { *value *= extent / total; }
        }
    }
    match (vertical, values) {
        (true,Some(values)) => rows.insert(index,values),
        (true,None) => { rows.remove(index); },
        (false,Some(values)) => for (row,value) in rows.iter_mut().zip(values) { row.insert(index,value); },
        (false,None) => for row in rows { row.remove(index); },
    }
    validate_element(&element)?; Ok(element)
}

pub(crate) fn default_fill(row: usize) -> &'static str { if row == 0 {"@accent1"} else if row % 2 == 0 {"@lt2"} else {"@lt1"} }
pub(crate) fn default_color(row: usize) -> &'static str { if row == 0 {"@lt1"} else {"@dk1"} }

impl CellStyle {
    pub(crate) fn text(&self, text: &str) -> TextFormat {
        let mut format = self.text_format.clone().unwrap_or_default();
        if let Some(style) = &self.text_style {
            if format.paragraphs.is_empty() {
                format.paragraphs = text.split('\n').map(|line| crate::rich_text::RichParagraph { runs: vec![crate::rich_text::RichRun { text:line.into(),style:style.clone(),field:None }], ..Default::default() }).collect();
            } else {
                for paragraph in &mut format.paragraphs { for run in &mut paragraph.runs { let mut combined = style.clone(); combined.overlay(&run.style); run.style = combined; } }
            }
        }
        format
    }
}

pub(crate) fn merge_flags(format: &TableFormat, row: usize, column: usize) -> (usize,usize,bool,bool) {
    format.merge_at(row,column).map_or((1,1,false,false), |region| (if column == region.column {region.col_span} else {1}, if row == region.row {region.row_span} else {1}, column != region.column, row != region.row))
}

fn native_number(node: roxmltree::Node<'_, '_>, name: &str, default: f64) -> Result<f64> {
    let value = node.attribute(name).map(|value| value.parse::<f64>().map_err(|_| Error::Invalid(format!("invalid table {name}")))).transpose()?.unwrap_or(default);
    if !value.is_finite() { return Err(Error::Invalid(format!("non-finite table {name}"))); } Ok(value)
}

fn native_flags(cell: roxmltree::Node<'_, '_>) -> Result<(usize,usize,bool,bool)> {
    let span = |name| cell.attribute(name).unwrap_or("1").parse::<usize>().map_err(|_| Error::Invalid("invalid table span".into()));
    let flag = |name| match cell.attribute(name).unwrap_or("0") { "0" | "false" => Ok(false), "1" | "true" => Ok(true), _ => Err(Error::Invalid("invalid table merge flag".into())) };
    Ok((span("gridSpan")?,span("rowSpan")?,flag("hMerge")?,flag("vMerge")?))
}

fn native_fill(node: roxmltree::Node<'_, '_>) -> Option<String> {
    use crate::native::{child,A};
    if child(node,A,"noFill").is_some() { return Some("none".into()); }
    let fill = child(node,A,"solidFill")?;
    if let Some(color) = child(fill,A,"srgbClr").and_then(|node| node.attribute("val")) { return Some(color.into()); }
    child(fill,A,"schemeClr").and_then(|node| node.attribute("val")).map(|value| format!("@{}",match value {"bg1"=>"lt1","bg2"=>"lt2","tx1"=>"dk1","tx2"=>"dk2",value=>value}))
}

fn style_difference(mut style: RunStyle, base: &RunStyle) -> RunStyle {
    macro_rules! omit { ($($name:ident),*) => { $(if style.$name == base.$name { style.$name = None; })* }; }
    omit!(bold,italic,underline,font_size,color,font_family,baseline,highlight,language); style
}

pub(crate) fn read_native(table: roxmltree::Node<'_, '_>, width: f64, height: f64) -> Result<(Vec<Vec<String>>,f64,TableFormat)> {
    use crate::native::{child,A,body_text};
    let native_rows: Vec<_> = table.children().filter(|node| node.has_tag_name((A,"tr"))).collect();
    let grid = child(table,A,"tblGrid").ok_or_else(|| Error::Invalid("native table grid missing".into()))?;
    let columns: Vec<_> = grid.children().filter(|node| node.has_tag_name((A,"gridCol"))).collect();
    if !(1..=12).contains(&native_rows.len()) || !(1..=8).contains(&columns.len()) { return Err(Error::Limit("native table exceeds 12 rows or 8 columns".into())); }
    let mut rows = Vec::new(); let mut cells = Vec::new();
    for row in &native_rows {
        let row_cells: Vec<_> = row.children().filter(|node| node.has_tag_name((A,"tc"))).collect();
        if row_cells.len() != columns.len() { return Err(Error::Invalid("native table must contain the full rectangular grid".into())); }
        rows.push(row_cells.iter().map(|cell| child(*cell,A,"txBody").map(body_text).unwrap_or_default()).collect::<Vec<_>>()); cells.extend(row_cells);
    }
    validate_rows(&rows)?;
    let mut sizes = std::collections::BTreeMap::new();
    for run in table.descendants().filter(|node| node.has_tag_name((A,"rPr"))) {
        let size = native_number(run,"sz",1500.0)?;
        if (600.0..=9000.0).contains(&size) { *sizes.entry(size.round() as u32).or_insert(0) += 1; }
    }
    let size = sizes.into_iter().max_by_key(|(_,count)| *count).map_or(20.0,|(size,_)| size as f64 / 75.0);
    let read_tracks = |nodes: &[roxmltree::Node<'_, '_>], attribute: &str, extent: f64| -> Result<Option<Dimensions>> {
        let values = nodes.iter().map(|node| native_number(*node,attribute,0.0).map(|value| value / 9525.0)).collect::<Result<Vec<_>>>()?;
        let unit = if (values.iter().sum::<f64>() - extent).abs() <= 0.001 {DimensionUnit::Absolute} else {DimensionUnit::Relative};
        let dimensions = Dimensions {unit,values}; dimensions.resolve(nodes.len(),extent)?;
        Ok(if unit == DimensionUnit::Absolute && dimensions.values.iter().all(|value| (value - extent / nodes.len() as f64).abs() < 0.001) {None} else {Some(dimensions)})
    };
    let mut format = TableFormat { column_widths:read_tracks(&columns,"w",width)?,row_heights:read_tracks(&native_rows,"h",height)?,..Default::default() };
    for (index,cell) in cells.iter().enumerate() {
        let (col_span,row_span,horizontal,vertical) = native_flags(*cell)?;
        if !horizontal && !vertical && (col_span > 1 || row_span > 1) { format.merges.push(MergeRegion {row:index / columns.len(),column:index % columns.len(),row_span,col_span}); }
    }
    format.validate(&rows,width,height)?;
    for (index,cell) in cells.iter().enumerate() {
        let row = index / columns.len(); let column = index % columns.len();
        if native_flags(*cell)? != merge_flags(&format,row,column) { return Err(Error::Invalid("inconsistent native table merge grid".into())); }
        let mut style = CellStyle::default();
        if let Some(properties) = child(*cell,A,"tcPr") {
            style.fill = native_fill(properties).filter(|fill| fill != default_fill(row));
            let padding = CellPadding {left:native_number(properties,"marL",91440.0)? / 9525.0,right:native_number(properties,"marR",91440.0)? / 9525.0,top:native_number(properties,"marT",45720.0)? / 9525.0,bottom:native_number(properties,"marB",45720.0)? / 9525.0};
            if [padding.left,padding.right,padding.top,padding.bottom].iter().any(|value| (*value - 6.0).abs() > 0.0001) { style.padding = Some(padding); }
            style.vertical = match properties.attribute("anchor").unwrap_or("t") {"t"=>None,"ctr"=>Some(VerticalAlign::Middle),"b"=>Some(VerticalAlign::Bottom),_=>return Err(Error::Unsupported("table vertical alignment".into()))};
            let outlines = ["lnL","lnR","lnT","lnB"].iter().map(|tag| child(properties,A,tag).map(|line| -> Result<_> { Ok(CellOutline {color:native_fill(line).filter(|color| color != "none").unwrap_or_else(|| "@dk1".into()),width:if child(line,A,"noFill").is_some() {0.0} else {native_number(line,"w",9525.0)? / 9525.0}}) }).transpose()).collect::<Result<Vec<_>>>()?;
            if outlines.iter().all(|outline| outline.is_some() && *outline == outlines[0]) { style.outline = outlines[0].clone(); }
        }
        if let Some(body) = child(*cell,A,"txBody") {
            let base = RunStyle::frame(size,default_color(row),row == 0,&TextFormat::default());
            if let Ok(mut paragraphs) = crate::native::read_rich_paragraphs(body,&base,&TextFormat::default()) {
                for paragraph in &mut paragraphs {
                    if paragraph.alignment == Some(crate::model::TextAlign::Left) { paragraph.alignment = None; }
                    if paragraph.bullet == Some(crate::model::Bullet::None) { paragraph.bullet = None; }
                    if paragraph.line_spacing == Some(crate::rich_text::Spacing::Percent(115000)) { paragraph.line_spacing = None; }
                    for run in &mut paragraph.runs { run.style = style_difference(run.style.clone(),&base); }
                }
                let simple = paragraphs.iter().all(|paragraph| paragraph.runs.len() == 1 && {
                    let mut properties = paragraph.clone(); properties.runs.clear(); properties == crate::rich_text::RichParagraph::default()
                });
                let first = paragraphs.first().and_then(|paragraph| paragraph.runs.first()).map(|run| run.style.clone()).unwrap_or_default();
                if simple && paragraphs.iter().all(|paragraph| paragraph.runs[0].style == first) {
                    if first != RunStyle::default() { style.text_style = Some(first); }
                } else if !paragraphs.is_empty() { style.text_format = Some(TextFormat { paragraphs,..Default::default() }); }
            }
        }
        if style != CellStyle::default() { format.cells.push(CellFormat {row,column,style}); }
    }
    format.validate(&rows,width,height)?;
    Ok((rows,size,format))
}

fn native_fragment(xml: &str, node: roxmltree::Node<'_, '_>) -> Result<String> {
    let mut value = xml[node.range()].to_owned();
    let mut reader = quick_xml::Reader::from_str(&value);
    let event = reader.read_event().map_err(|_| Error::Invalid("table text fragment".into()))?;
    let start = match event {quick_xml::events::Event::Start(start) | quick_xml::events::Event::Empty(start)=>start,_=>return Err(Error::Invalid("table text start element missing".into()))};
    let position = 1 + start.name().as_ref().len();
    let attributes = start.attributes().map(|attribute| attribute.map(|attribute| attribute.key.as_ref().to_vec()).map_err(|_| Error::Invalid("table text namespace attributes".into()))).collect::<Result<Vec<_>>>()?;
    let mut declarations = String::new();
    for namespace in node.namespaces() {
        if namespace.name() == Some("xml") { continue; }
        let name = namespace.name().map_or_else(|| "xmlns".into(),|prefix| format!("xmlns:{prefix}"));
        if !attributes.iter().any(|attribute| attribute == name.as_bytes()) { declarations.push_str(&format!(" {name}=\"{}\"",quick_xml::escape::escape(namespace.uri()))); }
    }
    value.insert_str(position,&declarations); Ok(value)
}

pub(crate) fn patch_paragraphs(xml: &str, body: roxmltree::Node<'_, '_>, generated: &str, next_body: roxmltree::Node<'_, '_>, edits: &mut Vec<(std::ops::Range<usize>,String)>) -> Result<()> {
    use crate::native::{child,A};
    crate::native::read_rich_paragraphs(body,&RunStyle::default(),&TextFormat::default())?;
    let original: Vec<_> = body.children().filter(|node| node.has_tag_name((A,"p"))).collect();
    let next: Vec<_> = next_body.children().filter(|node| node.has_tag_name((A,"p"))).collect();
    for (index,paragraph) in original.iter().enumerate() {
        let replacement = if let Some(next) = next.get(index) {
            let mut replacement = native_fragment(generated,*next)?;
            if let Some(end) = child(*paragraph,A,"endParaRPr") {
                let parsed = crate::pptx::parse(&replacement)?;
                let range = child(parsed.root_element(),A,"endParaRPr").map(|node| node.range());
                if let Some(range) = range { replacement.replace_range(range,&native_fragment(xml,end)?); }
            }
            replacement
        } else { String::new() };
        edits.push((paragraph.range(),replacement));
    }
    if next.len() > original.len() {
        let added = next[original.len()..].iter().map(|node| native_fragment(generated,*node)).collect::<Result<Vec<_>>>()?.concat();
        crate::native_save::insert_child(xml,body,&added,None,edits)?;
    }
    Ok(())
}
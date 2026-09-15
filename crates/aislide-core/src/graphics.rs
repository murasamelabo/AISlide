use crate::{model::{Element, Connection, valid_text}, Error, Result};

pub fn create_diagram(id: &str, steps: &[String]) -> Result<Element> {
    valid_text(id, 50)?;
    if id.is_empty() || steps.len() < 2 || steps.len() > 6 { return Err(Error::Invalid("a diagram requires an ID and 2-6 steps".into())); }
    for step in steps { valid_text(step, 80)?; }
    let gap = 42.0;
    let width = (1152.0 - gap * (steps.len() - 1) as f64) / steps.len() as f64;
    let mut children = Vec::new();
    for (index, step) in steps.iter().enumerate() {
        let left = index as f64 * (width + gap);
        let shape_id = format!("{id}-node-{index}");
        children.push(Element::Rect { id: shape_id.clone(), x: left, y: 52.0, width, height: 178.0, fill: if index % 2 == 0 { "EAF3EF" } else { "F8EDE9" }.into() });
        children.push(Element::Text { id: format!("{id}-text-{index}"), x: left + 16.0, y: 76.0, width: width - 32.0, height: 130.0, text: step.clone(), font_size: 22.0, color: "202525".into(), bold: true, format: Default::default() });
        if index + 1 < steps.len() {
            children.push(Element::Connector { id: format!("{id}-link-{index}"), x: left + width, y: 141.0, width: gap, height: 1.0, color: "586563".into(), stroke_width: 2.0, arrow: true, flip_v: false,
                start: Some(Connection { element_id: shape_id, site: 3 }), end: Some(Connection { element_id: format!("{id}-node-{}", index + 1), site: 1 }), routing: None });
        }
    }
    Ok(Element::Group { id: id.into(), x: 64.0, y: 230.0, width: 1152.0, height: 300.0, view_width: 1152.0, view_height: 300.0, children })
}